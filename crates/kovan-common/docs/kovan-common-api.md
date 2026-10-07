# Crate Documentation

**Version:** 0.0.2

**Format Version:** 60

# Module `kovan_common`

# kovan-common

Shared canonical types for the KOVAN knowledge layer. Every other KOVAN
crate depends on this one and speaks in these types; cross-crate links
(a symbol referencing a document, a benchmark referencing a validation
case) are expressed as the string IDs defined here rather than as direct
crate-to-crate dependencies.

**Source-of-truth rule:** these Rust structs are authoritative. BibTeX,
TOML, and Markdown metadata are *generated* from them and must never be
treated as the canonical record.

## What belongs here

Types that more than one KOVAN crate needs: documents, symbols,
repositories, correlations, benchmarks, validation cases, generated-code
provenance, and the small enums/records they contain. Do **not** put
pipeline logic (PDF parsing, semantic extraction, code generation) here —
that lives in the respective feature crate. The one exception is
[`zotero`] (2026-10-07, GitHub #748): Zotero's item model comes with the
conversions that define it (schema validation, CSL-JSON both ways, dates),
placed here by maintainer direction so every kovan crate can read and
write Zotero libraries.

## Module map

- [`document`] — [`KovanDocument`] + [`KovanDocumentBuilder`], [`Author`],
  [`Visibility`], [`DocumentType`].
- [`symbol`] — [`KovanSymbol`], [`KovanRepository`], the [`Language`] enum.
- [`knowledge`] — [`KovanCorrelation`], [`KovanBenchmark`],
  [`KovanValidationCase`], [`GeneratedArtifact`].
- [`code_map`], [`call_graph`], [`geometry`], [`mindmap_view`] — the
  pure data and layout behind the code map and the Code Review UI, moved
  out of `kovan` on 2026-10-06 so the wasm web view (`kovan-web`, GitHub
  #736) can use them. Plain `serde` + `std`; `kovan` re-exports each one
  under its old path.
- [`zotero`] — Zotero's item model, schema, CSL-JSON conversion and the
  [`KovanDocument`] mapping (GitHub #748), ported from Zotero (AGPL-3.0).

Everything is re-exported at the crate root, so downstream crates can keep
importing `kovan_common::KovanDocument` directly.

## Maturity

Unlike the other `kovan-*` crates, this one is **not** a placeholder stage
with stub logic — it is a plain data crate (types + serde derives + a
builder + convenience constructors) and there is nothing left here to stub
out. Every public type is fully implemented, documented, and round-trip
tested (`serde_json` and `toml`). The pipeline crates that build on top of
these types (~~`kovan-literature`,~~ `kovan-semantics`, `kovan-codegen`) still
carry their own `// TODO(kovan)` markers for unimplemented behaviour; that
is expected and tracked separately in each of those crates.
**CORRECTED 2026-09-25** — `kovan-literature/src` no longer contains any
`TODO(kovan)` marker (checked with `grep -rn 'TODO(kovan)'`); its
`DECISIONS.md` records the five stubs as fleshed out. `kovan-semantics`
(`src/adapters/`) and `kovan-codegen` (`src/macros_support.rs`) still do.

## Modules

## Module `document`

The canonical literature document type and its ergonomic builder.

[`KovanDocument`] is the single source of truth for one piece of literature
(KOVAN's "Canonical Representation" rule — BibTeX/TOML/Markdown are generated
*from* it, never authoritative). Because the struct is large, construct it
with [`KovanDocumentBuilder`] (via [`KovanDocument::builder`]) rather than by
field-by-field mutation.

```rust
pub mod document { /* ... */ }
```

### Types

#### Enum `Visibility`

Whether a piece of content may be redistributed (committed) or must stay
local to the user's machine.

```rust
pub enum Visibility {
    Open,
    Proprietary,
}
```

##### Variants

###### `Open`

Redistributable content — NRC reports, arXiv papers, open-access
journals, public theses. May be committed to version control.

###### `Proprietary`

User-owned content — textbooks, paywalled or proprietary reports.
Must remain local and must never be committed.

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
    fn clone(self: &Self) -> Visibility { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &Visibility) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `DocumentType`

The kind of literature a [`KovanDocument`] represents.

```rust
pub enum DocumentType {
    Paper,
    Report,
    Standard,
    Benchmark,
    Manual,
    Thesis,
    Other,
}
```

##### Variants

###### `Paper`

Journal or conference paper, preprint.

###### `Report`

Technical report (e.g. an NRC/NUREG report).

###### `Standard`

A standard or code (e.g. ASME, IEEE, ISO).

###### `Benchmark`

A benchmark specification (e.g. an ICSBEP evaluation).

###### `Manual`

A user manual or software manual.

###### `Thesis`

A doctoral or master's thesis / dissertation (e.g. a UC Berkeley
eScholarship deposit). Distinct from [`DocumentType::Report`] because the
citation form differs: a thesis cites its awarding institution, not an
issuing organisation and report number.

###### `Other`

Anything else; refine into a dedicated variant when a real need appears.

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
    fn clone(self: &Self) -> DocumentType { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &DocumentType) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Author`

A single author of a document.

Also used for organisational "authors" (e.g. a standards body or a
benchmark evaluation group) — by convention, set `family` to the
organisation's name and leave `given` as an empty string (see
`examples/build_document.rs`, which does this for an ICSBEP evaluation).

```rust
pub struct Author {
    pub family: String,
    pub given: String,
    pub affiliation: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `family` | `String` | Family name (surname), or the full name for an organisational author. |
| `given` | `String` | Given name(s). Empty for an organisational author. |
| `affiliation` | `Option<String>` | Affiliation/institution, free text for now. `None` when unknown or<br>not applicable (e.g. for an organisational author, where the<br>organisation is already named in `family`). |

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
    fn clone(self: &Self) -> Author { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &Author) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `KovanDocument`

The canonical KOVAN document — the single source of truth for one piece of
literature. Everything else (BibTeX, generated Markdown, indices) is derived
from this struct.

The struct is intentionally wide (identity, classification, bibliographic
metadata, journal locators, source provenance, generated assets, and cross
links). Prefer [`KovanDocument::builder`] over field-by-field mutation for
readable construction.

```rust
pub struct KovanDocument {
    pub id: String,
    pub slug: String,
    pub visibility: Visibility,
    pub document_type: DocumentType,
    pub title: String,
    pub authors: Vec<Author>,
    pub abstract_text: String,
    pub year: Option<u32>,
    pub doi: Option<String>,
    pub journal: Option<String>,
    pub institution: Option<String>,
    pub publisher: Option<String>,
    pub volume: Option<String>,
    pub pages: Option<String>,
    pub number: Option<String>,
    pub keywords: Vec<String>,
    pub tags: Vec<String>,
    pub source_url: Option<String>,
    pub source_path: Option<String>,
    pub source_sha256: Option<String>,
    pub page_count: Option<u32>,
    pub assets: Vec<String>,
    pub related_symbols: Vec<String>,
    pub related_repositories: Vec<String>,
    pub related_benchmarks: Vec<String>,
    pub markdown_body: String,
    pub zotero_item: Option<crate::zotero::ZoteroItem>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | Stable unique identifier (e.g. a content hash or an assigned key). |
| `slug` | `String` | Human-friendly URL/file-safe slug. |
| `visibility` | `Visibility` | Open vs proprietary; governs whether it may be committed. |
| `document_type` | `DocumentType` | The kind of document. |
| `title` | `String` | Full title. |
| `authors` | `Vec<Author>` | Ordered list of authors. Empty if unknown or not yet catalogued. |
| `abstract_text` | `String` | Abstract text (plain text). Empty string if unknown or not yet<br>extracted from the source (not `Option` — an absent abstract is<br>indistinguishable from an unextracted one at this stage, and callers<br>should treat both the same way). |
| `year` | `Option<u32>` | Publication year, if known. |
| `doi` | `Option<String>` | Digital Object Identifier, if any. |
| `journal` | `Option<String>` | Journal name, if a journal paper. `None` for report/standard/manual<br>document types, where it does not apply. |
| `institution` | `Option<String>` | Institution, if a report/thesis. `None` for paper/standard document<br>types, where it does not apply. |
| `publisher` | `Option<String>` | Publisher, if applicable. `None` if unknown or not applicable. |
| `volume` | `Option<String>` | Journal volume for a journal article (a `String` because volumes are<br>not always numeric, e.g. `"12A"`). `None` when unknown or not a journal<br>article. |
| `pages` | `Option<String>` | Page range or article number within the volume (e.g. `"110439"` or<br>`"245-260"`). `String` because it may be a hyphenated range or an<br>electronic article id. `None` when unknown. |
| `number` | `Option<String>` | Journal issue number (e.g. `"3"`). `String` for the same reason as<br>[`KovanDocument::volume`]. `None` when unknown or not applicable. |
| `keywords` | `Vec<String>` | Free-form keywords (author- or extraction-supplied). Empty if none<br>were recorded. |
| `tags` | `Vec<String>` | KOVAN-internal tags (curated by the local user/tooling, distinct from<br>`keywords`). Empty if none have been applied. |
| `source_url` | `Option<String>` | Where the source PDF/record came from, if recorded (e.g. a download<br>URL). `None` for locally authored or unattributed documents. |
| `source_path` | `Option<String>` | Path to the source file this document was ingested from (the on-disk<br>PDF, relative or absolute per the caller). `None` before ingestion or<br>for locally authored documents. |
| `source_sha256` | `Option<String>` | Lowercase-hex SHA-256 of the source file's bytes, for provenance /<br>change detection. `None` when the hash has not been computed (the<br>`kovan-literature` pipeline currently leaves this `None` — see its<br>`DECISIONS.md`). |
| `page_count` | `Option<u32>` | Number of pages in the source document, if known (e.g. counted from the<br>PDF). `None` before ingestion or when the count is unavailable. |
| `assets` | `Vec<String>` | Paths (relative to the generated-assets root) of files extracted from<br>the source, such as figures. Empty if none were extracted or the<br>asset pass has not run. |
| `related_symbols` | `Vec<String>` | IDs of related [`crate::KovanSymbol`]s (e.g. a code symbol implementing<br>a correlation this document derives). Empty if none are linked yet. |
| `related_repositories` | `Vec<String>` | IDs of related [`crate::KovanRepository`]s. Empty if none are linked yet. |
| `related_benchmarks` | `Vec<String>` | IDs of related [`crate::KovanBenchmark`]s. Empty if none are linked yet. |
| `markdown_body` | `String` | The document body as Markdown (generated from the source PDF). Empty<br>string before the PDF-import pipeline has run (see `kovan-literature`). |
| `zotero_item` | `Option<crate::zotero::ZoteroItem>` | The Zotero item this document was imported from, verbatim (GitHub<br>#748), so that exporting back to Zotero loses nothing kovan does not<br>hold. `None` for documents that did not come from Zotero, and for every<br>document written before 2026-10-07: the field is optional, defaults to<br>`None` and is not written when `None`, so existing JSON/TOML files load<br>and serialise exactly as before. See [`crate::zotero::kovan`]. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(id: impl Into<String>, slug: impl Into<String>, visibility: Visibility, document_type: DocumentType, title: impl Into<String>) -> Self { /* ... */ }
  ```
  Create an otherwise-empty document with the required identity and

- ```rust
  pub fn builder</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(id: impl Into<String>, slug: impl Into<String>, visibility: Visibility, document_type: DocumentType, title: impl Into<String>) -> KovanDocumentBuilder { /* ... */ }
  ```
  Start building a document from its required identity fields. Chain the

- ```rust
  pub fn from_zotero_item(item: &ZoteroItem) -> KovanDocument { /* ... */ }
  ```
  Shorthand for [`ZoteroItem::to_kovan_document`].

- ```rust
  pub fn to_zotero_item(self: &Self) -> ZoteroItem { /* ... */ }
  ```
  Shorthand for [`ZoteroItem::from_kovan_document`].

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
    fn clone(self: &Self) -> KovanDocument { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &KovanDocument) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `KovanDocumentBuilder`

Ergonomic builder for [`KovanDocument`].

Owns the document it is assembling by value (no lifetimes, no trait objects,
per the workspace rules); each setter takes and returns `self`. Every field
has a sensible empty default, so [`KovanDocumentBuilder::build`] is
infallible. Obtain one from [`KovanDocument::builder`].

```rust
pub struct KovanDocumentBuilder {
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
  pub fn author(self: Self, author: Author) -> Self { /* ... */ }
  ```
  Append a single author (call repeatedly to add several, in order).

- ```rust
  pub fn authors(self: Self, authors: Vec<Author>) -> Self { /* ... */ }
  ```
  Replace the entire author list.

- ```rust
  pub fn abstract_text</* synthetic */ impl Into<String>: Into<String>>(self: Self, text: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the plain-text abstract.

- ```rust
  pub fn year(self: Self, year: u32) -> Self { /* ... */ }
  ```
  Set the publication year.

- ```rust
  pub fn doi</* synthetic */ impl Into<String>: Into<String>>(self: Self, doi: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the DOI.

- ```rust
  pub fn journal</* synthetic */ impl Into<String>: Into<String>>(self: Self, journal: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the journal name.

- ```rust
  pub fn institution</* synthetic */ impl Into<String>: Into<String>>(self: Self, institution: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the institution.

- ```rust
  pub fn publisher</* synthetic */ impl Into<String>: Into<String>>(self: Self, publisher: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the publisher.

- ```rust
  pub fn volume</* synthetic */ impl Into<String>: Into<String>>(self: Self, volume: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the journal volume.

- ```rust
  pub fn pages</* synthetic */ impl Into<String>: Into<String>>(self: Self, pages: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the page range / article number.

- ```rust
  pub fn number</* synthetic */ impl Into<String>: Into<String>>(self: Self, number: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the journal issue number.

- ```rust
  pub fn keywords(self: Self, keywords: Vec<String>) -> Self { /* ... */ }
  ```
  Replace the keyword list.

- ```rust
  pub fn tags(self: Self, tags: Vec<String>) -> Self { /* ... */ }
  ```
  Replace the tag list.

- ```rust
  pub fn source_url</* synthetic */ impl Into<String>: Into<String>>(self: Self, url: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the source URL the document was fetched from.

- ```rust
  pub fn source_path</* synthetic */ impl Into<String>: Into<String>>(self: Self, path: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the on-disk path of the source file this document was ingested from.

- ```rust
  pub fn source_sha256</* synthetic */ impl Into<String>: Into<String>>(self: Self, sha256: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the lowercase-hex SHA-256 of the source file's bytes.

- ```rust
  pub fn page_count(self: Self, pages: u32) -> Self { /* ... */ }
  ```
  Set the source page count.

- ```rust
  pub fn assets(self: Self, assets: Vec<String>) -> Self { /* ... */ }
  ```
  Replace the generated-asset path list.

- ```rust
  pub fn related_symbols(self: Self, ids: Vec<String>) -> Self { /* ... */ }
  ```
  Replace the related-symbol ID list.

- ```rust
  pub fn related_repositories(self: Self, ids: Vec<String>) -> Self { /* ... */ }
  ```
  Replace the related-repository ID list.

- ```rust
  pub fn related_benchmarks(self: Self, ids: Vec<String>) -> Self { /* ... */ }
  ```
  Replace the related-benchmark ID list.

- ```rust
  pub fn markdown_body</* synthetic */ impl Into<String>: Into<String>>(self: Self, body: impl Into<String>) -> Self { /* ... */ }
  ```
  Set the generated Markdown body.

- ```rust
  pub fn zotero_item(self: Self, item: crate::zotero::ZoteroItem) -> Self { /* ... */ }
  ```
  Attach the Zotero item the document was imported from.

- ```rust
  pub fn build(self: Self) -> KovanDocument { /* ... */ }
  ```
  Finish building and return the assembled [`KovanDocument`]. Infallible —

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
    fn clone(self: &Self) -> KovanDocumentBuilder { /* ... */ }
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

- **RefUnwindSafe**
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
## Module `knowledge`

Knowledge-graph provenance types: correlations, benchmarks, validation
cases, and generated-code provenance.

These express the "Paper → Correlation → Implementation → Validation" chain
(`docs/kovan.md`, "Integration Vision") as records linked by the string IDs
defined across `kovan-common`, rather than by direct object references.

```rust
pub mod knowledge { /* ... */ }
```

### Types

#### Struct `KovanCorrelation`

An engineering correlation (e.g. a Nusselt-number correlation) linking a
literature source to an implementation and validation evidence.

```rust
pub struct KovanCorrelation {
    pub id: String,
    pub name: String,
    pub source_document_id: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | Stable identifier. |
| `name` | `String` | Human-readable name (e.g. `"Dittus-Boelter"`). |
| `source_document_id` | `Option<String>` | ID of the [`crate::KovanDocument`] this correlation is sourced from.<br>`None` if the source has not been catalogued yet. |

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
    fn clone(self: &Self) -> KovanCorrelation { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &KovanCorrelation) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `KovanBenchmark`

A benchmark specification (e.g. an ICSBEP critical-experiment evaluation).

```rust
pub struct KovanBenchmark {
    pub id: String,
    pub name: String,
    pub source_document_id: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | Stable identifier. |
| `name` | `String` | Display name (e.g. `"HEU-MET-FAST-001 (Godiva)"`). |
| `source_document_id` | `Option<String>` | ID of the [`crate::KovanDocument`] describing the benchmark, if any.<br>`None` if the source evaluation has not been catalogued yet. |

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
    fn clone(self: &Self) -> KovanBenchmark { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &KovanBenchmark) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `KovanValidationCase`

A validation case tying an implementation to a benchmark and its measured
result — the provenance record KOVAN ultimately aims to produce.

The measured result itself (e.g. `k_eff = 1.00042 ± 0.00015`) is
intentionally not modelled here yet: the shape of that data depends on
what kind of case it is (a k-eigenvalue benchmark vs. a correlation
accuracy check vs. a thermal-hydraulic transient comparison have
different result schemas), and no consumer of this type exists yet to
drive that design. Adding it speculatively would risk guessing wrong;
see `DECISIONS.md`.

```rust
pub struct KovanValidationCase {
    pub id: String,
    pub name: String,
    pub benchmark_id: Option<String>,
    pub implementation_symbol_id: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | Stable identifier. |
| `name` | `String` | Display name (e.g. `"TUAS Dittus-Boelter vs. Godiva Nu correlation"`). |
| `benchmark_id` | `Option<String>` | ID of the [`KovanBenchmark`] this case is validated against, if any.<br>`None` if this case validates a [`KovanCorrelation`] directly instead<br>(not every validation case is benchmark-based). |
| `implementation_symbol_id` | `Option<String>` | ID of the [`crate::KovanSymbol`] (the code implementing the correlation<br>or model under test) exercised by this case, if any. `None` if the<br>implementation has not been linked yet. |

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
    fn clone(self: &Self) -> KovanValidationCase { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &KovanValidationCase) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `GeneratedArtifact`

Provenance record for a piece of code emitted by `kovan-codegen`.

This closes the "Correlation → Implementation" link of the KOVAN integration
vision: it records *what* numerical method was generated, the generated
source, and (optionally) which [`KovanCorrelation`] it implements and which
[`crate::KovanDocument`] the method derives from. It carries no behaviour —
it is a serialisable audit record so a generated kernel can be traced back to
the paper and correlation it came from.

```rust
pub struct GeneratedArtifact {
    pub method: String,
    pub source: String,
    pub correlation_id: Option<String>,
    pub source_document_id: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `method` | `String` | The numerical method that was generated, as a stable label. Produced by<br>`kovan-codegen` from its `Method` enum (e.g. `"Ode(Rk4)"`). Free text<br>here so `kovan-common` need not depend on the codegen catalogue enums. |
| `source` | `String` | The generated Rust source, verbatim as `kovan-codegen` emitted it. May<br>be an empty string when only the provenance link is being recorded (the<br>source is stored elsewhere). |
| `correlation_id` | `Option<String>` | ID of the [`KovanCorrelation`] this generated code implements, if the<br>generation was tied to one. `None` for a bare method generation. |
| `source_document_id` | `Option<String>` | ID of the [`crate::KovanDocument`] (the paper/report) the method derives<br>from, if known. `None` when the source has not been linked. |

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
    fn clone(self: &Self) -> GeneratedArtifact { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &GeneratedArtifact) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
## Module `symbol`

Repository and symbol types shared across the KOVAN semantics layer.

[`KovanSymbol`] is the normalised, cross-language representation of one
source symbol (a function, type, module, …). [`Language`] is the closed set
of source languages KOVAN understands; owning it here means the semantics
crate does not need a crate-local language identity type on the symbol.

```rust
pub mod symbol { /* ... */ }
```

### Types

#### Enum `Language`

A source language KOVAN understands and normalises symbols from.

This is the language *identity* only — it says nothing about which tool is
used to analyse it. `kovan-semantics` keeps a separate `LanguageAdapter`
enum for tool/extension selection (rust-analyzer vs. clangd, which file
extensions belong to the language, …) and converts into this shared
[`Language`] when it builds a [`KovanSymbol`].

```rust
pub enum Language {
    Rust,
    Cpp,
    Python,
    Fortran,
}
```

##### Variants

###### `Rust`

Rust.

###### `Cpp`

C++ (and C headers scanned alongside it).

###### `Python`

Python.

###### `Fortran`

Fortran.

##### Implementations

###### Methods

- ```rust
  pub fn as_str(self: Self) -> &'static str { /* ... */ }
  ```
  A short, stable, human-readable label (`"Rust"`, `"C++"`, `"Python"`,

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
    fn clone(self: &Self) -> Language { /* ... */ }
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

- **Eq**
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
    fn eq(self: &Self, other: &Language) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `KovanSymbol`

A semantic symbol extracted from a source repository (a function, type,
module, …). Normalised across languages by `kovan-semantics`.

Carries enough to locate the symbol in its repository: the `qualified_name`
for identity, plus `file` + `line` for the exact definition site and
[`language`](KovanSymbol::language) for how it was parsed.

```rust
pub struct KovanSymbol {
    pub id: String,
    pub qualified_name: String,
    pub kind: String,
    pub repository_id: String,
    pub file: String,
    pub line: u32,
    pub language: Language,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | Stable identifier for the symbol. |
| `qualified_name` | `String` | Fully-qualified name/path as reported by the language tooling (e.g.<br>`tuas_boussinesq_solver::heat_transfer_correlations::nusselt::dittus_boelter`). |
| `kind` | `String` | Symbol kind, free text for now (e.g. `"fn"`, `"struct"`, `"class"`).<br>Kept as free text rather than an enum because it is sourced directly<br>from heterogeneous language tooling (rust-analyzer, clangd, Pyright,<br>fortls) whose vocabularies don't line up cleanly; normalise into an<br>enum here only once `kovan-semantics` actually needs to match on it. |
| `repository_id` | `String` | ID of the [`crate::KovanRepository`] this symbol belongs to. |
| `file` | `String` | Repository-relative path to the file the symbol is defined in (e.g.<br>`src/nozzle.rs`). Empty string only if the extractor could not attribute<br>a file (it always can for the ripgrep-first path). |
| `line` | `u32` | 1-based line number of the definition's keyword line within<br>[`file`](KovanSymbol::file). |
| `language` | `Language` | The source language this symbol was extracted from. |

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
    fn clone(self: &Self) -> KovanSymbol { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &KovanSymbol) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `KovanRepository`

A source-code repository KOVAN understands (e.g. TUAS, OpenFOAM, NJOY).

```rust
pub struct KovanRepository {
    pub id: String,
    pub name: String,
    pub language: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | Stable identifier. |
| `name` | `String` | Display name. |
| `language` | `String` | Primary language, free text for now (e.g. `"Rust"`, `"C++"`,<br>`"Fortran"`). Kept a `String` rather than [`Language`] because a<br>repository can be polyglot and the "primary" label is a curated,<br>human-facing description, not a parser selection. |

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
    fn clone(self: &Self) -> KovanRepository { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &KovanRepository) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
## Module `call_graph`

The workspace call graph's data model, crate -> module -> function
(GitHub #737), and its per-crate split for the web (#736). Moved here
from `kovan::call_graph` on 2026-10-06; `kovan` re-exports it.
The workspace **call graph** at three levels, crate → module (or example)
→ function, with each function's source text, as deterministic data for
the code-review UI (GitHub #737, part of #735).

`kovan-cli call-graph` builds it (`commands::call_graph`, which resolves
calls through rust-analyzer with `code-walk`'s machinery) and writes it as
JSON. web-kovan reads that JSON as a static file; desktop kovan can build
it live. This module is the data model and the pure assembly step: plain
`serde` + `std`, no GUI, no I/O, no rust-analyzer, so it can move to a
wasm-clean crate unchanged. ~~can move~~ **Moved 2026-10-06** to
`kovan_common::call_graph` (web-kovan, #736); `kovan::call_graph`
re-exports it. [`split`] cuts a document into one file per crate for the
web.

# Shape

```text
CallGraphDoc
  crates[]         name, dir, maturity (the Cargo.toml tag)
    targets[]      the lib, then each example (kind, name, root file)
      modules[]    one per source file in the target's module tree
        functions[]  id, lines, signature, doc, source, unresolved calls
  calls[]          function -> function, every call-site line
  module_calls[]   module (file) -> module, call sites and function pairs
  crate_calls[]    crate -> crate, the same counts
  outside[]        call targets in workspace code outside the scope
  totals           counts of all of the above
```

# Schema 2 (2026-10-06, GitHub #746): additions only

Every schema-1 field keeps its name, type and meaning; a reader that
ignores unknown fields reads schema 2 unchanged. Added (all omitted from
the JSON when empty):

```text
CallGraphDoc.commit            HEAD the data was built at
CallGraphDoc.site_base         base URL of Citation.site paths
CrateGraph.tests[]             integration-test targets (tests/*.rs),
                               Target with kind "test"; same shape
Module.upstream                {style, line, project, repository, version,
                                commit, source, files[], licence, url}
                               from the attribution header; url only when
                               repository (github/gitlab) and commit are
                               both recorded: see [`upstream`]
Module.upstream_unparsed       a provenance marker that could not be read
Module.history[]               {sha, date, author, subject}, newest first,
                               at most 10: see [`history`]
Module.concepts[]              `//! kovan-concept:` tags
Function.test_fn               carries #[test] (an entry point)
Function.reached_by            {tests_total, tests[{hops, id}],
                                examples_total,
                                examples[{hops, crate, example, via}]},
                               nearest 10 of each; resolved calls only,
                               so a LOWER BOUND: see [`reach`]
Function.cited_by[]            {kind: code_walk|concept_tag|relation,
                                page, line, anchor, site}: see [`citations`]
Totals.{test_targets, test_fns, non_test_functions, reached_by_tests,
  reached_by_examples, upstream_files, upstream_links, upstream_unparsed,
  cited_functions, citations, history_files}
```

# Schema 3 (2026-10-07, GitHub #757): additions only

```text
Call.kind "operator"           a call through an operator (`a + b`,
                               `v[i]`, `-x`) to a workspace impl of the
                               operator trait; only the SCIP backend
                               finds these
CallGraphDoc.generator         {backend: lsp|scip, rust_analyzer}: how the
                               calls were resolved, and the rust-analyzer
                               that resolved them (SCIP's output is not
                               stable across versions)
```

A schema-2 document reads unchanged (test
`schema_2_documents_still_load`, on a document written by the schema-2
code); a schema-2 *reader* meets an unknown `"operator"` kind, which is
why the version moved.

# Function ids

A function's `id` is `code-walk`'s form, `path/to/file.rs::name` for a
free function and `path/to/file.rs::Type::name` for a method (trait-impl
methods use the self type, trait declarations the trait name), so review
stamps (#739) and `code-walk --from` use the same key. When that form is
not unique in its file (two `impl`s of different traits both defining
`fmt` on one type, or a nested helper sharing a name), every one of them
gets `#k` appended, `k` counting from 1 in source order, and
`ambiguous: true`; [`function_ids`] is the rule.

# Determinism

[`CallGraphDoc::assemble`] sorts everything (crates by name, targets lib
first then examples by name, modules by file, functions by line, calls by
`(from, to, kind)` with their lines ascending, the aggregates by key) and
uses only `BTreeMap`s, so the same inputs in any order give
byte-identical JSON (test `assembly_is_order_independent`). Nothing
machine-specific (absolute paths, timings) is stored.

```rust
pub mod call_graph { /* ... */ }
```

### Modules

## Module `citations`

**Docs that cite a function** (GitHub #746, item 7): which Markdown pages
reference it, read from the pages, plus `kovan-concept:` tags in source.

# Sources

- **`code-walk` blocks** in lessons, deep dives and tutorials
  (`<!-- code-walk: from=… to=… -->` … `<!-- /code-walk -->`, see
  `commands::code_walk::lesson`). **Every function on the walk** is
  recorded, not only the endpoints: the generated body lists each hop as
  `<!-- snippet-check: <file>:<line> fn <name> -->`, which names the
  function by file and declaration line; the header's `from=`, `to=` and
  each `hand:` hop name functions in code-walk path form. A block whose
  body has not been generated yet still contributes its header's ids.
- **`kovan-concept:` tags** in source: `//! kovan-concept: <path>` in a
  module's doc comment tags the module ([`super::Module::concepts`]);
  `/// kovan-concept: <path>` in a function's doc comment tags the
  function, recorded as a citation of kind `concept_tag`. **Count found
  2026-10-06: 0** (the tags are proposed in
  `crates/kovan-literature/docs/concept-proposals.md`, none added yet).
- **Reserved:** kovan Markdown artifacts' `[[relation]] to = "code:…"`
  links (#743), kind `relation`. Not read yet; the slot exists so the
  reader need not change when they are.

# Anchor and site link

`anchor` is the id mdBook gives the nearest heading above the block:
the heading's text with inline code, emphasis and link targets removed,
lower-cased, whitespace to `-`, keeping letters, digits, `_` and `-`
(mdBook's `normalize_id`); an explicit `{#id}` wins. mdBook's `-1`
suffix for a repeated heading is not reproduced. A page inside a book
listed in `docs/site/deep-dives.txt` or `docs/site/tutorials.txt` gets
`site`, its path on the Pages site (`deep-dives/<name>/<page>.html#<anchor>`,
`README.md` → `index.html`), relative to [`SITE_BASE`].

```rust
pub mod citations { /* ... */ }
```

### Types

#### Enum `CitationKind`

**Attributes:**

- `Other("#[serde(rename_all = \"snake_case\")]")`

```rust
pub enum CitationKind {
    CodeWalk,
    ConceptTag,
    Relation,
}
```

##### Variants

###### `CodeWalk`

###### `ConceptTag`

###### `Relation`

Reserved for #743's `[[relation]] to = "code:…"` links; not emitted.

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
    fn clone(self: &Self) -> CitationKind { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &CitationKind) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CitationKind) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &CitationKind) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Citation`

One reference to a function.

```rust
pub struct Citation {
    pub kind: CitationKind,
    pub page: String,
    pub line: u32,
    pub anchor: String,
    pub site: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kind` | `CitationKind` |  |
| `page` | `String` | The citing page, workspace-relative; for `concept_tag`, the concept<br>path the tag names. |
| `line` | `u32` | 1-based line of the block (or tag). |
| `anchor` | `String` | The heading id above the block; empty for a tag or a page without<br>headings. |
| `site` | `Option<String>` | Path on the Pages site, relative to [`SITE_BASE`]. |

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
    fn clone(self: &Self) -> Citation { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Citation) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Citation) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Citation) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `FnRef`

How a page names a function.

```rust
pub enum FnRef {
    At {
        file: String,
        line: u32,
        name: String,
    },
    Path(String),
}
```

##### Variants

###### `At`

`snippet-check`: workspace-relative file, 1-based line, name.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `file` | `String` |  |
| `line` | `u32` |  |
| `name` | `String` |  |

###### `Path`

Code-walk path form, `file.rs::name` or `file.rs::Type::name`.

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FnRef { /* ... */ }
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &FnRef) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FnRef) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &FnRef) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `PageRefs`

One code-walk block's functions.

```rust
pub struct PageRefs {
    pub line: u32,
    pub anchor: String,
    pub refs: Vec<FnRef>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `line` | `u32` | 1-based line of the opening comment. |
| `anchor` | `String` |  |
| `refs` | `Vec<FnRef>` |  |

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
    fn clone(self: &Self) -> PageRefs { /* ... */ }
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
    fn eq(self: &Self, other: &PageRefs) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `heading_id`

mdBook's heading id for `text`.

```rust
pub fn heading_id(text: &str) -> String { /* ... */ }
```

#### Function `scan_page`

The code-walk blocks of one Markdown page.

```rust
pub fn scan_page(text: &str) -> Vec<PageRefs> { /* ... */ }
```

#### Function `concept_tags`

The `kovan-concept:` tags in a source file: `(0-based line, doc kind
'!' for `//!` or '/' for `///`, concept path)`.

```rust
pub fn concept_tags(lines: &[String]) -> Vec<(u32, char, String)> { /* ... */ }
```

#### Function `site_path`

The site path of a page, given the books as `(prefix, book dir)` pairs
(`("deep-dives/triso-atops", "crates/boon-lay/docs/lessons")`).

```rust
pub fn site_path(books: &[(String, String)], page: &str, anchor: &str) -> Option<String> { /* ... */ }
```

#### Function `parse_books`

The books of a `docs/site/*.txt` list (`<url-name> <dir>` lines, `#`
comments), each under `prefix/`.

```rust
pub fn parse_books(text: &str, prefix: &str) -> Vec<(String, String)> { /* ... */ }
```

### Constants and Statics

#### Constant `SITE_BASE`

The workspace's GitHub Pages site (`.github/workflows/pages.yml`).

```rust
pub const SITE_BASE: &str = "https://theodoreonzgit.github.io/outram-park-backend/";
```

## Module `compare`

Edge-by-edge comparison of two call-graph documents (GitHub #757): the
instrument that judges the SCIP backend against the LSP one, and that a
CI check can use to compare a regenerated graph with a committed one.

Edges are matched by `(from, to)` function id. Both backends take their
ids from the same source scanner, so an id names the same function in
both documents; the call kind is compared separately (an edge one
backend calls `call` and the other `fn_value` is still the same edge).
Unresolved calls are matched by `(function, line, kind)`.

Pure data, no I/O.

```rust
pub mod compare { /* ... */ }
```

### Types

#### Struct `EdgeCounts`

Edge counts for one caller crate (or the total).

```rust
pub struct EdgeCounts {
    pub both: usize,
    pub same_lines: usize,
    pub kind_differs: usize,
    pub only_a: usize,
    pub only_b: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `both` | `usize` | Edges in both documents. |
| `same_lines` | `usize` | Of those, the ones whose call-site lines are identical. |
| `kind_differs` | `usize` | Of those, the ones whose call kind differs. |
| `only_a` | `usize` |  |
| `only_b` | `usize` |  |

##### Implementations

###### Methods

- ```rust
  pub fn recall_of_a(self: &Self) -> f64 { /* ... */ }
  ```
  `both / (both + only_a)`: the share of A's edges that B has.

- ```rust
  pub fn jaccard(self: &Self) -> f64 { /* ... */ }
  ```
  `both / (both + only_a + only_b)`.

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
    fn clone(self: &Self) -> EdgeCounts { /* ... */ }
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
    fn default() -> EdgeCounts { /* ... */ }
    ```

- **Eq**
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
    fn eq(self: &Self, other: &EdgeCounts) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `EdgeOnly`

An edge found in one document only.

```rust
pub struct EdgeOnly {
    pub from: String,
    pub to: String,
    pub kind: super::CallKind,
    pub lines: Vec<u32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `from` | `String` |  |
| `to` | `String` |  |
| `kind` | `super::CallKind` |  |
| `lines` | `Vec<u32>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn is_self_edge(self: &Self) -> bool { /* ... */ }
  ```
  A function calling itself.

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
    fn clone(self: &Self) -> EdgeOnly { /* ... */ }
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &EdgeOnly) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &EdgeOnly) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &EdgeOnly) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `UnresolvedCounts`

Unresolved-call counts for one kind (`trait`, `closure`, ...).

```rust
pub struct UnresolvedCounts {
    pub a: usize,
    pub b: usize,
    pub both: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `a` | `usize` |  |
| `b` | `usize` |  |
| `both` | `usize` | Same function, line and kind in both. |

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
    fn clone(self: &Self) -> UnresolvedCounts { /* ... */ }
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
    fn default() -> UnresolvedCounts { /* ... */ }
    ```

- **Eq**
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
    fn eq(self: &Self, other: &UnresolvedCounts) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `Comparison`

The comparison of document A (the reference) with document B.

```rust
pub struct Comparison {
    pub functions_a: usize,
    pub functions_b: usize,
    pub functions_both: usize,
    pub total: EdgeCounts,
    pub per_crate: std::collections::BTreeMap<String, EdgeCounts>,
    pub only_a: Vec<EdgeOnly>,
    pub only_b: Vec<EdgeOnly>,
    pub unresolved: std::collections::BTreeMap<String, UnresolvedCounts>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `functions_a` | `usize` |  |
| `functions_b` | `usize` |  |
| `functions_both` | `usize` | Function ids in both. |
| `total` | `EdgeCounts` |  |
| `per_crate` | `std::collections::BTreeMap<String, EdgeCounts>` | By the caller's crate. |
| `only_a` | `Vec<EdgeOnly>` | Sorted. |
| `only_b` | `Vec<EdgeOnly>` |  |
| `unresolved` | `std::collections::BTreeMap<String, UnresolvedCounts>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn only_by_kind(edges: &[EdgeOnly]) -> BTreeMap<CallKind, usize> { /* ... */ }
  ```
  Counts of `only_a` (or `only_b`) by call kind.

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
    fn clone(self: &Self) -> Comparison { /* ... */ }
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
    fn default() -> Comparison { /* ... */ }
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
    fn eq(self: &Self, other: &Comparison) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `compare`

Compares `a` (the reference) with `b`.

```rust
pub fn compare(a: &super::CallGraphDoc, b: &super::CallGraphDoc) -> Comparison { /* ... */ }
```

## Module `history`

**Recent history** of each source file (GitHub #746, item 8): the last
[`HISTORY_LEN`] commits that touched it, from **one** `git log` pass over
the scope rather than one call per file.

The command runs (see [`GIT_LOG_ARGS`])

```text
git -c core.quotepath=off log --no-merges --no-renames --no-color \
    --format=%x1e%H%x1f%aI%x1f%an%x1f%s --name-only HEAD -- <crate dirs>
```

and [`parse`] keeps, per path, the first [`HISTORY_LEN`] commits in the
order git prints them (newest first by commit date, a fixed traversal for
a given HEAD), so the result is deterministic for a given commit.
`--no-renames` makes a rename list both paths, independent of the
user's `diff.renames`; history is per path and does not follow a file
across a rename. Merge commits are skipped (they list no files without
`-m`). No diffs are stored: per-function history and the diff since a
review stamp are computed by the reader from the two raw files at the
two commits.

```rust
pub mod history { /* ... */ }
```

### Types

#### Struct `CommitRef`

One commit that touched a file.

```rust
pub struct CommitRef {
    pub sha: String,
    pub date: String,
    pub author: String,
    pub subject: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `sha` | `String` | Full hash. |
| `date` | `String` | Author date, ISO 8601 with offset as recorded. |
| `author` | `String` |  |
| `subject` | `String` |  |

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
    fn clone(self: &Self) -> CommitRef { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &CommitRef) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
### Functions

#### Function `parse`

Path -> its newest [`HISTORY_LEN`] commits, from `git log` output in the
[`GIT_LOG_ARGS`] format.

```rust
pub fn parse(out: &str) -> std::collections::BTreeMap<String, Vec<CommitRef>> { /* ... */ }
```

### Constants and Statics

#### Constant `HISTORY_LEN`

Commits kept per file.

```rust
pub const HISTORY_LEN: usize = 10;
```

#### Constant `GIT_LOG_ARGS`

The `git` arguments before the pathspec.

```rust
pub const GIT_LOG_ARGS: &[&str] = _;
```

## Module `modules`

The module tree of one Cargo target, read from source text: which `mod`
declarations a file makes, which file each one loads, and which line
ranges are test-only code.

Pure functions over a file's lines. The caller supplies the source lines
as read (`raw`) and the same lines with comments, literals and attributes
blanked to spaces (`code`, as `commands::code_walk::source::blank_non_code`
produces them), so a `mod` inside a comment or a string is not a
declaration, while attributes (`#[cfg(test)]`, `#[path = "…"]`) are read
from `raw`.

# File resolution (the Rust reference, "Modules", 2018 edition and later)

For `mod x;` in file `F`:

- `F` is a **mod-rs file** when it is a target's root (`lib.rs`,
  `main.rs`, an example's root) or is named `mod.rs`: `x` is looked up in
  `F`'s directory. Otherwise (`a/b.rs`) it is looked up in `a/b/`.
- Inside inline modules (`mod a { mod x; }`) each enclosing name adds a
  directory: `…/a/x.rs`.
- `x.rs` is preferred to `x/mod.rs`; the first that exists wins.
- `#[path = "p"]` on a declaration outside any inline module is relative
  to `F`'s own directory; inside inline modules it is relative to the
  directory those modules name. (The reference's finer rule for
  `#[path]` inside an inline module of a non-mod-rs file is not
  reproduced; no crate in this workspace uses it.)

Inline modules themselves are not separate entries in the call graph:
their functions belong to the file's module. A `#[cfg(test)]` inline
module marks its line range as test code ([`test_ranges`]).

```rust
pub mod modules { /* ... */ }
```

### Types

#### Struct `ModDecl`

An out-of-line `mod name;` declaration found in a file.

```rust
pub struct ModDecl {
    pub name: String,
    pub inline: Vec<String>,
    pub path_attr: Option<String>,
    pub test: bool,
    pub line: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` |  |
| `inline` | `Vec<String>` | Names of the inline modules the declaration sits in, outermost first. |
| `path_attr` | `Option<String>` | The `#[path = "…"]` attribute's value, if any. |
| `test` | `bool` | `#[cfg(test)]` (or a `cfg` naming `test`) on the declaration or on an<br>enclosing inline module. |
| `line` | `u32` | 0-based line of the `mod` keyword. |

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
    fn clone(self: &Self) -> ModDecl { /* ... */ }
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
    fn eq(self: &Self, other: &ModDecl) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `FileModules`

What [`scan`] finds in one file.

```rust
pub struct FileModules {
    pub decls: Vec<ModDecl>,
    pub test_ranges: Vec<(u32, u32)>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `decls` | `Vec<ModDecl>` | Out-of-line declarations, in source order. |
| `test_ranges` | `Vec<(u32, u32)>` | 0-based inclusive line ranges of `#[cfg(test)]` inline module bodies. |

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
    fn clone(self: &Self) -> FileModules { /* ... */ }
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
    fn default() -> FileModules { /* ... */ }
    ```

- **Eq**
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
    fn eq(self: &Self, other: &FileModules) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `is_test_cfg`

Whether attribute text gates the item on `test`: `#[cfg(test)]`,
`#[cfg(all(test, …))]` and the like (not `cfg(not(test))`).

```rust
pub fn is_test_cfg(attrs: &str) -> bool { /* ... */ }
```

#### Function `is_test_attr`

Whether attribute text marks a function as a test (`#[test]`,
`#[tokio::test]`, `#[rstest]`, …).

```rust
pub fn is_test_attr(attrs: &str) -> bool { /* ... */ }
```

#### Function `scan`

Every `mod` declaration in a file, tracking inline modules by brace depth.

```rust
pub fn scan(raw: &[String], code: &[String]) -> FileModules { /* ... */ }
```

#### Function `candidate_files`

The files `decl` (found in `file`) may load, in the order Rust tries
them. `mod_rs` says whether `file` is a mod-rs file (a target root or a
`mod.rs`).

```rust
pub fn candidate_files(file: &str, mod_rs: bool, decl: &ModDecl) -> Vec<String> { /* ... */ }
```

#### Function `is_mod_rs`

Whether a file loaded by a `mod` declaration is a mod-rs file.

```rust
pub fn is_mod_rs(file: &str) -> bool { /* ... */ }
```

## Module `reach`

**Tests and examples that reach a function** (GitHub #746, part 1): the
call graph walked forwards from every test and every example, recorded
on each function they reach.

# Entry points

- A **test** is a function carrying a test attribute (`#[test]`,
  `#[tokio::test]`, `#[rstest]`: [`super::Function::test_fn`]), in the
  lib's `#[cfg(test)]` code, in an example, or in an integration-test
  target under `tests/` (schema 2 builds those; see
  [`super::CrateGraph::tests`]). Helpers in test code are walked through
  but are not entry points.
- An **example** is a whole example target: every non-test function in it
  is a starting point at hop 0, because an egui example's code is mostly
  entered through trait callbacks (`eframe::App::update`) that the graph
  does not resolve, so a walk from `main` alone would miss most of it.
  `hops` is the distance from the nearest function of the example, and
  `via` names that function (the smallest id at that distance).

# What is recorded

On every non-test function reached by at least one entry: the number of
tests and of examples that reach it, and the nearest [`REACH_CAP`] of
each, sorted by `(hops, id)`. A direct call from a test is 1 hop. An
example's own functions are not listed as reached by that example.

# A lower bound

The walk follows only **resolved** calls (`calls`, both direct calls and
function values). Calls the tool marked `UNRESOLVED` (closures, trait
methods, derived methods, macro bodies) are not followed, and calls into
functions outside the scope stop there. So a function with no entry
here may still be exercised by a test; the lists are a lower bound.

```rust
pub mod reach { /* ... */ }
```

### Types

#### Struct `Reach`

The entries that reach one function.

```rust
pub struct Reach {
    pub tests_total: usize,
    pub tests: Vec<TestReach>,
    pub examples_total: usize,
    pub examples: Vec<ExampleReach>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tests_total` | `usize` | All tests that reach the function. |
| `tests` | `Vec<TestReach>` | The nearest [`REACH_CAP`], by `(hops, id)`. |
| `examples_total` | `usize` | All example targets that reach the function. |
| `examples` | `Vec<ExampleReach>` |  |

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
    fn clone(self: &Self) -> Reach { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &Reach) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `TestReach`

```rust
pub struct TestReach {
    pub hops: u32,
    pub id: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `hops` | `u32` |  |
| `id` | `String` | The test function's id. |

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
    fn clone(self: &Self) -> TestReach { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &TestReach) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TestReach) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &TestReach) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `ExampleReach`

```rust
pub struct ExampleReach {
    pub hops: u32,
    pub krate: String,
    pub example: String,
    pub via: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `hops` | `u32` |  |
| `krate` | `String` |  |
| `example` | `String` | The example target's name. |
| `via` | `String` | The example's function nearest to this one. |

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
    fn clone(self: &Self) -> ExampleReach { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &ExampleReach) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ExampleReach) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &ExampleReach) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
### Functions

#### Function `fill`

Fills `reached_by` on every non-test function of `doc`.

```rust
pub fn fill(doc: &mut super::CallGraphDoc) { /* ... */ }
```

### Constants and Statics

#### Constant `REACH_CAP`

How many of the nearest tests (and examples) each function lists.

```rust
pub const REACH_CAP: usize = 10;
```

## Module `split`

The call graph cut into **one file per crate**, for the web (web-kovan,
GitHub #736 / #738).

The whole-workspace [`CallGraphDoc`] is too large to download at once
(10.5 MB for two crates, #737), so `kovan-cli call-graph --split-dir`
writes:

```text
index.json      SplitIndex: every crate's counts and file size, and every
                review stamp's state (small)
<crate>.json    CrateSlice: the crate's targets, module tree and functions
                WITHOUT source text, the calls into and out of it, the
                module and crate aggregates touching it, and a per-file
                `links` slot (empty for now)
```

and `search.json` ([`SearchIndex`]): every crate, module and function by
name and path, compact, for the page's search bar (loaded up front).

The web page fetches `index.json` and `search.json` at start and a `<crate>.json` when a
crate is opened or expanded.

# No source text (maintainer, 2026-10-06)

Source is **not** shipped: each function keeps its file, its line range,
its signature and its first doc sentence, and the page fetches the file
itself on demand from the repository at the commit the site was built
from (`raw.githubusercontent.com/<repo>/<commit>/<file>`), then slices the
function's lines. Measured 2026-10-06 on boon-lay (914 functions): the
slice is 2.27 MB pretty with source, 1.22 MB pretty without, and smaller
again compact (the form written here; the report of #736 has the number).

# Links in the source (reserved, empty)

[`SourceFile::links`] is where definition and reference links for the
identifiers in a file will go (token range -> target id), to be filled
from rust-analyzer's whole-workspace index (`rust-analyzer scip`, #745).
Nothing fills it yet; the slot exists so that the UI and the file layout
do not change when it is filled.

# Determinism

Pure functions of the document: same document, byte-identical files
(`BTreeMap`s and the document's own sorted order only).

```rust
pub mod split { /* ... */ }
```

### Types

#### Struct `SplitIndex`

`index.json`.

```rust
pub struct SplitIndex {
    pub schema: u32,
    pub graph_schema: u32,
    pub crates: Vec<IndexEntry>,
    pub totals: super::Totals,
    pub stamps: Vec<StampState>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `schema` | `u32` |  |
| `graph_schema` | `u32` | The [`super::SCHEMA_VERSION`] of the document that was split. |
| `crates` | `Vec<IndexEntry>` |  |
| `totals` | `super::Totals` | The whole document's totals. |
| `stamps` | `Vec<StampState>` | Every review stamp's state (`review/stamps.toml`, #739), judged when<br>the files were written. Empty when there are no stamps. |

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
    fn clone(self: &Self) -> SplitIndex { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &SplitIndex) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `IndexEntry`

One crate in [`SplitIndex`].

```rust
pub struct IndexEntry {
    pub name: String,
    pub dir: String,
    pub file: String,
    pub bytes: u64,
    pub modules: usize,
    pub functions: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` |  |
| `dir` | `String` |  |
| `file` | `String` | `<name>.json`, relative to the index. |
| `bytes` | `u64` | Bytes of `file`, so the page can say what it loads. |
| `modules` | `usize` |  |
| `functions` | `usize` |  |

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
    fn clone(self: &Self) -> IndexEntry { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &IndexEntry) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `CrateSlice`

`<crate>.json`: one crate with every call that starts or ends in it.

```rust
pub struct CrateSlice {
    pub schema: u32,
    pub krate: super::CrateGraph,
    pub calls: Vec<super::Call>,
    pub module_calls: Vec<super::AggregateCall>,
    pub crate_calls: Vec<super::AggregateCall>,
    pub outside: Vec<super::OutsideFn>,
    pub commit: Option<String>,
    pub site_base: Option<String>,
    pub files: Vec<SourceFile>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `schema` | `u32` |  |
| `krate` | `super::CrateGraph` | The crate, every function's `source` emptied (module doc). |
| `calls` | `Vec<super::Call>` | Calls whose caller or callee is in this crate. |
| `module_calls` | `Vec<super::AggregateCall>` | Module aggregates with either end in this crate. |
| `crate_calls` | `Vec<super::AggregateCall>` | Crate aggregates with either end this crate. |
| `outside` | `Vec<super::OutsideFn>` | The out-of-scope workspace functions this crate's calls reach. |
| `commit` | `Option<String>` | Schema 2: the commit the graph was built at, and the site base the<br>`cited_by[].site` paths are relative to. |
| `site_base` | `Option<String>` |  |
| `files` | `Vec<SourceFile>` | One per module file, sorted by file. |

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
    fn clone(self: &Self) -> CrateSlice { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &CrateSlice) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `SourceFile`

Per-file data of a [`CrateSlice`].

```rust
pub struct SourceFile {
    pub file: String,
    pub links: Vec<SourceLink>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `file` | `String` |  |
| `links` | `Vec<SourceLink>` | Identifier links in this file. **Reserved, always empty today** (see<br>the module doc). |

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
    fn clone(self: &Self) -> SourceFile { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &SourceFile) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `SourceLink`

One identifier in a source file and where it leads (reserved).

```rust
pub struct SourceLink {
    pub line: u32,
    pub col_start: u32,
    pub col_end: u32,
    pub target: String,
    pub kind: LinkKind,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `line` | `u32` | 1-based line. |
| `col_start` | `u32` | 0-based character columns, end exclusive. |
| `col_end` | `u32` |  |
| `target` | `String` | A call-graph function id, or another symbol id. |
| `kind` | `LinkKind` |  |

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
    fn clone(self: &Self) -> SourceLink { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &SourceLink) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `LinkKind`

**Attributes:**

- `Other("#[serde(rename_all = \"snake_case\")]")`

```rust
pub enum LinkKind {
    Definition,
    Reference,
}
```

##### Variants

###### `Definition`

###### `Reference`

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
    fn clone(self: &Self) -> LinkKind { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &LinkKind) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `StampVerdict`

**Attributes:**

- `Other("#[serde(rename_all = \"snake_case\")]")`

A stamp's state as the web shows it.

```rust
pub enum StampVerdict {
    Valid,
    Stale,
}
```

##### Variants

###### `Valid`

The function still hashes to the stamped hash.

###### `Stale`

Void: the function changed (or is gone) since it was stamped.

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
    fn clone(self: &Self) -> StampVerdict { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &StampVerdict) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `StampState`

One stamp, judged (from `kovan::review_stamps::check`).

```rust
pub struct StampState {
    pub function: String,
    pub verdict: StampVerdict,
    pub reason: String,
    pub rung: u8,
    pub reviewer: String,
    pub date: String,
    pub note: String,
    pub permalink: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `function` | `String` | The stamped function's id (code-walk path form). |
| `verdict` | `StampVerdict` |  |
| `reason` | `String` | Why it is stale; empty when valid. |
| `rung` | `u8` |  |
| `reviewer` | `String` |  |
| `date` | `String` | `YYYY-MM-DD`. |
| `note` | `String` |  |
| `permalink` | `String` | The code as it was stamped. |

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
    fn clone(self: &Self) -> StampState { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &StampState) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `SearchIndex`

`search.json`: names and paths of everything, for the search bar.
Tuples serialise as JSON arrays, which keeps the file small.

```rust
pub struct SearchIndex {
    pub schema: u32,
    pub crates: Vec<String>,
    pub modules: Vec<SearchModule>,
    pub functions: Vec<SearchFn>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `schema` | `u32` |  |
| `crates` | `Vec<String>` | Crate names; a [`SearchModule`]'s first field indexes this. |
| `modules` | `Vec<SearchModule>` |  |
| `functions` | `Vec<SearchFn>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn build(doc: &CallGraphDoc) -> SearchIndex { /* ... */ }
  ```
  Build from the whole document (test modules and functions left out).

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
    fn clone(self: &Self) -> SearchIndex { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &SearchIndex) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `SearchModule`

`(crate index, file, module path)`; the path is `""` for a target root.

```rust
pub struct SearchModule(pub usize, pub String, pub String);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |
| 1 | `String` |  |
| 2 | `String` |  |

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
    fn clone(self: &Self) -> SearchModule { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &SearchModule) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `SearchFn`

`(module index, id after "<file>::", line)`: the function id is
`"{file}::{suffix}"`. Test functions are left out.

```rust
pub struct SearchFn(pub usize, pub String, pub u32);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `usize` |  |
| 1 | `String` |  |
| 2 | `u32` |  |

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
    fn clone(self: &Self) -> SearchFn { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &SearchFn) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
### Functions

#### Function `split`

Cut `doc` into one [`CrateSlice`] per crate, in the document's order.

```rust
pub fn split(doc: &super::CallGraphDoc) -> Vec<CrateSlice> { /* ... */ }
```

#### Function `to_json`

Compact JSON with a trailing newline, the form every split file is
written in. Compact, not pretty: these files are downloaded, never read
or committed.

```rust
pub fn to_json<T: Serialize>(v: &T) -> String { /* ... */ }
```

#### Function `split_files`

The serialised files of a split: `(file name, contents)`, the index last.
`stamps` is stored in the index as given.

```rust
pub fn split_files(doc: &super::CallGraphDoc, stamps: Vec<StampState>) -> Vec<(String, String)> { /* ... */ }
```

### Constants and Statics

#### Constant `SPLIT_SCHEMA`

Version of the split layout; bumped on any breaking change.

```rust
pub const SPLIT_SCHEMA: u32 = 1;
```

## Module `upstream`

The **upstream counterpart** of a ported source file, read from its
attribution header (GitHub #746, part 2).

The workspace rule (root `CLAUDE.md`, "Preserve GPLv3 compatibility and
provenance headers") is that a file porting from an upstream project
keeps an attribution header naming the upstream project, source file,
version or commit, copyright and licence. This module reads that header
into an [`Upstream`] record. Plain `std`, no I/O.

# What is read

Only the file's **leading `//` comment block**: the lines before the
first line that is not a `//` comment or blank (`//!` and `///` doc
comments end it). Provenance written only in a `//!` doc comment ("Ported
from `bsigma` in NJOY2016 `broadr.f90`", most of `njoy-outram-park-fork`)
and attribution blocks further down a file (a separator-headed block
introducing one ported routine) are not read; [`scan`] reports the first
as [`Scan::Unparsed`] when the doc comment says "ported from", "port of"
or "upstream commit" (a flag for a person to look at, which also catches
prose such as "a port of physics the maintainer has validated"). A
negated marker ("not a port of the PANAMA Fortran", boon-lay's
`fuel_failure/`) is not an attribution.

# The styles found (surveyed 2026-10-06 over `crates/`)

1. **Key-value**, `Upstream <key> : <value>`, aligned or not, with the
   value continuing on more-indented lines. Keys seen: `project`, `URL`,
   `repo`, `commit`, `version`, `source`, `source files`, `file`,
   `licence`/`license`, `copyright`, `author`; also a bare `Upstream:
   <url>` (GeN-Foam ports). Examples: `boon-lay` `triso_atops_fork/`
   (TRISO-ATOPS), `changi` `flexpart/` (FLEXPART, commit inside the
   version: `10.4 (2019-11-12), commit 3d7eebf`), `raffles` `scram/`,
   `outram-foam-*` GeN-Foam and OpenFOAM-dev ports, `cyclus` ports.
2. **Prose**, `Ported from <project> …` / `Derived from <project> (<url>),
   upstream commit <sha>` / `PORTED from …`. Examples: `njoy-outram-park-fork`
   (``Ported from NJOY2016 `src/groupr.f90` (git commit ac5adf5…)``),
   `outram-park-fork-offbeat` (`Derived from OFFBEAT (https://gitlab.com/…),
   upstream commit 80e8445…`), `outram-mc-libs`/`outram-blender`
   (`Ported from OpenMC (https://github.com/openmc-dev/openmc, …): src/mesh.cpp`),
   `petir` (`Ported from the GNU Scientific Library `cheb/` module (cheb/init.c,
   …), GSL 2.8 at commit cf180cd…`).

Key-value fields win over prose where both are present.

# The link: never guessed

[`link`] builds a URL only when the header records **both** a repository
URL on `github.com` or `gitlab.com` (the two hosts whose URL scheme is
known) **and** a commit hash (7-40 hex digits; a version number is
recorded but not turned into a tag, since the tag's spelling is not
recorded). It links the file (`…/blob/<commit>/<path>`) when the header
names exactly one file with a directory part, else the tree at the commit.
A project with no recorded repository URL (NJOY2016's headers record the
commit but not the repository) gets its fields and no URL.

```rust
pub mod upstream { /* ... */ }
```

### Types

#### Enum `HeaderStyle`

**Attributes:**

- `Other("#[serde(rename_all = \"snake_case\")]")`

How the header was written.

```rust
pub enum HeaderStyle {
    KeyValue,
    Prose,
}
```

##### Variants

###### `KeyValue`

`Upstream <key>: <value>` lines.

###### `Prose`

`Ported from <project> …` / `Derived from <project> …` sentences.

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
    fn clone(self: &Self) -> HeaderStyle { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &HeaderStyle) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &HeaderStyle) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &HeaderStyle) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Upstream`

A file's upstream counterpart, as its attribution header records it.

```rust
pub struct Upstream {
    pub style: HeaderStyle,
    pub line: u32,
    pub project: Option<String>,
    pub repository: Option<String>,
    pub version: Option<String>,
    pub commit: Option<String>,
    pub source: Option<String>,
    pub files: Vec<String>,
    pub licence: Option<String>,
    pub url: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `style` | `HeaderStyle` |  |
| `line` | `u32` | 1-based line where the attribution starts. |
| `project` | `Option<String>` |  |
| `repository` | `Option<String>` | The repository URL as recorded (`<>` and a trailing `.git`/`/` removed). |
| `version` | `Option<String>` |  |
| `commit` | `Option<String>` | The commit hash as recorded (7-40 hex digits). |
| `source` | `Option<String>` | The recorded upstream source text, continuation lines joined. |
| `files` | `Vec<String>` | The file paths found in `source` (line suffixes `:12-40` removed). |
| `licence` | `Option<String>` |  |
| `url` | `Option<String>` | Set only by [`link`]'s rule: recorded repository on a known host and a<br>recorded commit. |

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
    fn clone(self: &Self) -> Upstream { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &Upstream) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `Scan`

What [`scan`] found in a file's header.

```rust
pub enum Scan {
    None,
    Parsed(Upstream),
    Unparsed(String),
}
```

##### Variants

###### `None`

No attribution marker: not a port, or not declared as one.

###### `Parsed`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Upstream` |  |

###### `Unparsed`

A provenance marker was seen but no project or repository could be
read; the string says where (`doc comment: …` or `header: …`).

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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Scan { /* ... */ }
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
    fn eq(self: &Self, other: &Scan) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `scan`

Reads the attribution header of a file's lines.

```rust
pub fn scan(lines: &[String]) -> Scan { /* ... */ }
```

#### Function `link`

The upstream URL, under the rule in the module doc: known host and a
recorded commit, else `None`.

```rust
pub fn link(up: &Upstream) -> Option<String> { /* ... */ }
```

### Types

#### Struct `CallGraphDoc`

The whole graph for one scope of crates.

```rust
pub struct CallGraphDoc {
    pub schema: u32,
    pub scope: Vec<String>,
    pub crates: Vec<CrateGraph>,
    pub calls: Vec<Call>,
    pub module_calls: Vec<AggregateCall>,
    pub crate_calls: Vec<AggregateCall>,
    pub outside: Vec<OutsideFn>,
    pub totals: Totals,
    pub commit: Option<String>,
    pub site_base: Option<String>,
    pub generator: Option<Generator>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `schema` | `u32` |  |
| `scope` | `Vec<String>` | The crates analysed, sorted. Calls into other workspace crates are<br>kept, their targets listed in `outside`. |
| `crates` | `Vec<CrateGraph>` |  |
| `calls` | `Vec<Call>` |  |
| `module_calls` | `Vec<AggregateCall>` |  |
| `crate_calls` | `Vec<AggregateCall>` |  |
| `outside` | `Vec<OutsideFn>` |  |
| `totals` | `Totals` |  |
| `commit` | `Option<String>` | Schema 2: the commit (`git rev-parse HEAD`) the data was built at;<br>absent outside a git checkout. |
| `site_base` | `Option<String>` | Schema 2: the base URL that `Citation::site` paths are relative to. |
| `generator` | `Option<Generator>` | Schema 3: how the calls were resolved, and by which rust-analyzer. |

##### Implementations

###### Methods

- ```rust
  pub fn assemble(crates: Vec<CrateGraph>, raw_calls: Vec<RawCall>, outside: Vec<OutsideFn>, external_calls: usize) -> CallGraphDoc { /* ... */ }
  ```
  Sorts and aggregates the builder's output into the document.

- ```rust
  pub fn merge(docs: Vec<CallGraphDoc>) -> CallGraphDoc { /* ... */ }
  ```
  One document from several built separately (one per crate, for the

- ```rust
  pub fn mixed_generators(docs: &[CallGraphDoc]) -> bool { /* ... */ }
  ```
  True when `docs` were resolved by different backends or

- ```rust
  pub fn to_json(self: &Self) -> String { /* ... */ }
  ```
  Pretty JSON with a trailing newline: the file `kovan-cli call-graph`

- ```rust
  pub fn functions(self: &Self) -> impl Iterator<Item = (&CrateGraph, &Module, &Function)> { /* ... */ }
  ```
  Every function, with the crate and module it is in.

- ```rust
  pub fn function(self: &Self, id: &str) -> Option<&Function> { /* ... */ }
  ```
  The function with this id, if it is in scope.

- ```rust
  pub fn calls_from<''a>(self: &'a Self, id: &'a str) -> impl Iterator<Item = &'a Call> + ''a { /* ... */ }
  ```
  The calls out of `id`, in `(to, kind)` order.

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
    fn clone(self: &Self) -> CallGraphDoc { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &CallGraphDoc) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Generator`

Schema 3 (#757): what resolved the calls.

```rust
pub struct Generator {
    pub backend: Backend,
    pub rust_analyzer: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `backend` | `Backend` |  |
| `rust_analyzer` | `String` | The rust-analyzer that resolved them: `rust-analyzer --version` for<br>the LSP backend, the index's own tool name and version for SCIP. |

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
    fn clone(self: &Self) -> Generator { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &Generator) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `Backend`

**Attributes:**

- `Other("#[serde(rename_all = \"snake_case\")]")`

Where call targets come from.

```rust
pub enum Backend {
    Lsp,
    Scip,
}
```

##### Variants

###### `Lsp`

rust-analyzer's LSP, one definition query per call-shaped token.

###### `Scip`

One `rust-analyzer scip` index of the workspace (#757).

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
    fn clone(self: &Self) -> Backend { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Backend) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Backend) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Backend) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `CrateGraph`

```rust
pub struct CrateGraph {
    pub name: String,
    pub dir: String,
    pub maturity: Option<u8>,
    pub targets: Vec<Target>,
    pub tests: Vec<Target>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` |  |
| `dir` | `String` | The crate's folder relative to the workspace root, `/`-separated. |
| `maturity` | `Option<u8>` | The crate's `[package.metadata.kovan]` maturity (0..=4), when tagged. |
| `targets` | `Vec<Target>` |  |
| `tests` | `Vec<Target>` | Schema 2: the integration-test targets (`tests/*.rs`, kind `test`),<br>kept apart from `targets` so a schema-1 reader is unaffected. |

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
    fn clone(self: &Self) -> CrateGraph { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &CrateGraph) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `TargetKind`

**Attributes:**

- `Other("#[serde(rename_all = \"snake_case\")]")`

```rust
pub enum TargetKind {
    Lib,
    Example,
    Test,
}
```

##### Variants

###### `Lib`

###### `Example`

###### `Test`

Schema 2: an integration-test target; only in `CrateGraph::tests`.

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
    fn clone(self: &Self) -> TargetKind { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &TargetKind) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &TargetKind) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &TargetKind) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Target`

```rust
pub struct Target {
    pub kind: TargetKind,
    pub name: String,
    pub root: String,
    pub modules: Vec<Module>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kind` | `TargetKind` |  |
| `name` | `String` | The Cargo target name (`boon_lay`, `htgr_sim_v1`). |
| `root` | `String` | The target's root file, workspace-relative. |
| `modules` | `Vec<Module>` |  |

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
    fn clone(self: &Self) -> Target { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &Target) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Module`

One source file of a target's module tree.

```rust
pub struct Module {
    pub file: String,
    pub path: String,
    pub parent: Option<String>,
    pub test: bool,
    pub maturity: Option<u8>,
    pub functions: Vec<Function>,
    pub upstream: Option<upstream::Upstream>,
    pub upstream_unparsed: Option<String>,
    pub history: Vec<history::CommitRef>,
    pub concepts: Vec<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `file` | `String` | Workspace-relative path; the module's id in `module_calls`. |
| `path` | `String` | Path from the target root, `physics::fission_product_release`; empty<br>for the root itself. |
| `parent` | `Option<String>` | The declaring module's file; `None` for the root. |
| `test` | `bool` | Declared under `#[cfg(test)]` (itself or an ancestor). |
| `maturity` | `Option<u8>` | From the crate tag: the deepest `maturity_modules` entry that is this<br>module or an ancestor, else the crate's level. Library modules only. |
| `functions` | `Vec<Function>` |  |
| `upstream` | `Option<upstream::Upstream>` | Schema 2: the upstream counterpart from the file's attribution<br>header ([`upstream`]); absent when the file has none. |
| `upstream_unparsed` | `Option<String>` | Schema 2: a provenance marker was seen but could not be parsed (the<br>reason and the line); no `upstream` is guessed. |
| `history` | `Vec<history::CommitRef>` | Schema 2: the newest commits that touched the file ([`history`]). |
| `concepts` | `Vec<String>` | Schema 2: `//! kovan-concept:` tags in the file ([`citations`]). |

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
    fn clone(self: &Self) -> Module { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &Module) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `FnKind`

**Attributes:**

- `Other("#[serde(rename_all = \"snake_case\")]")`

```rust
pub enum FnKind {
    Free,
    Method,
    TraitImpl,
    TraitDecl,
}
```

##### Variants

###### `Free`

A free function (also a function nested in another's body).

###### `Method`

In an inherent `impl Type`.

###### `TraitImpl`

In `impl Trait for Type`.

###### `TraitDecl`

A trait's own declaration (with or without a default body).

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
    fn clone(self: &Self) -> FnKind { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &FnKind) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FnKind) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &FnKind) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Function`

```rust
pub struct Function {
    pub id: String,
    pub ambiguous: bool,
    pub name: String,
    pub kind: FnKind,
    pub owner: Option<String>,
    pub trait_name: Option<String>,
    pub test: bool,
    pub test_fn: bool,
    pub start_line: u32,
    pub line: u32,
    pub end_line: u32,
    pub signature: String,
    pub doc: String,
    pub source: String,
    pub unresolved: Vec<Unresolved>,
    pub reached_by: Option<reach::Reach>,
    pub cited_by: Vec<citations::Citation>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` | `file.rs::name` or `file.rs::Type::name` (see the module doc), unique. |
| `ambiguous` | `bool` | Set when `id` needed a `#k` suffix to be unique. |
| `name` | `String` |  |
| `kind` | `FnKind` |  |
| `owner` | `Option<String>` | The self type (impls) or the trait (trait declarations). |
| `trait_name` | `Option<String>` | The trait of a trait impl. |
| `test` | `bool` | A `#[test]` function, or inside `#[cfg(test)]` code. |
| `test_fn` | `bool` | Schema 2: carries a test attribute (`#[test]`, `#[tokio::test]`,<br>`#[rstest]`): an entry point for [`reach`]. `test` is also set. |
| `start_line` | `u32` | 1-based first line: the start of the `///` doc comment and attributes<br>directly above the declaration, else the declaration. |
| `line` | `u32` | 1-based line of the `fn` identifier. |
| `end_line` | `u32` | 1-based line of the closing brace (the declaration line when there is<br>no body). |
| `signature` | `String` |  |
| `doc` | `String` | The first sentence of the doc comment; empty when undocumented. |
| `source` | `String` | Lines `start_line..=end_line` as written, joined with `\n`. |
| `unresolved` | `Vec<Unresolved>` | Every call in the body the tool could not follow, by line. |
| `reached_by` | `Option<reach::Reach>` | Schema 2: the tests and examples that reach this (non-test) function<br>through resolved calls, nearest first; a lower bound ([`reach`]). |
| `cited_by` | `Vec<citations::Citation>` | Schema 2: the pages (and concept tags) that cite this function<br>([`citations`]). |

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
    fn clone(self: &Self) -> Function { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &Function) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Unresolved`

A call `code-walk` marks `UNRESOLVED(<kind>)`: never guessed, never
dropped.

```rust
pub struct Unresolved {
    pub line: u32,
    pub kind: String,
    pub callee: String,
    pub detail: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `line` | `u32` | 1-based call-site line. |
| `kind` | `String` | `trait`, `closure`, `macro`, `no-definition` or `other`. |
| `callee` | `String` | The token at the call site. |
| `detail` | `String` |  |

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
    fn clone(self: &Self) -> Unresolved { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Unresolved) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Unresolved) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Unresolved) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `CallKind`

**Attributes:**

- `Other("#[serde(rename_all = \"snake_case\")]")`

```rust
pub enum CallKind {
    Call,
    FnValue,
    Operator,
}
```

##### Variants

###### `Call`

A call expression rust-analyzer resolved.

###### `FnValue`

A function passed by value (`.map(f)`), resolved by rust-analyzer.

###### `Operator`

Schema 3 (#757): an operator (`a + b`, `v[i]`, `-x`) that resolves
to a workspace `impl` of the operator trait (SCIP backend only).

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
    fn clone(self: &Self) -> CallKind { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &CallKind) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CallKind) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &CallKind) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Call`

`from` calls `to` at each of `lines` (1-based, in `from`'s file).

```rust
pub struct Call {
    pub from: String,
    pub to: String,
    pub kind: CallKind,
    pub lines: Vec<u32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `from` | `String` |  |
| `to` | `String` |  |
| `kind` | `CallKind` |  |
| `lines` | `Vec<u32>` |  |

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
    fn clone(self: &Self) -> Call { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &Call) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `AggregateCall`

Calls between two modules (by file) or two crates (by name).

```rust
pub struct AggregateCall {
    pub from: String,
    pub to: String,
    pub sites: usize,
    pub pairs: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `from` | `String` |  |
| `to` | `String` |  |
| `sites` | `usize` | Call sites. |
| `pairs` | `usize` | Distinct `(caller, callee)` function pairs. |

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
    fn clone(self: &Self) -> AggregateCall { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &AggregateCall) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `OutsideFn`

A workspace function outside the scope that a function in scope calls.

```rust
pub struct OutsideFn {
    pub id: String,
    pub krate: Option<String>,
    pub file: String,
    pub line: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `id` | `String` |  |
| `krate` | `Option<String>` | The workspace crate whose folder holds `file`, if any. |
| `file` | `String` |  |
| `line` | `u32` |  |

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
    fn clone(self: &Self) -> OutsideFn { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &OutsideFn) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Totals`

```rust
pub struct Totals {
    pub crates: usize,
    pub targets: usize,
    pub modules: usize,
    pub functions: usize,
    pub calls: usize,
    pub call_sites: usize,
    pub unresolved: usize,
    pub unresolved_by_kind: std::collections::BTreeMap<String, usize>,
    pub outside: usize,
    pub external_calls: usize,
    pub test_targets: usize,
    pub test_fns: usize,
    pub reached_by_tests: usize,
    pub reached_by_examples: usize,
    pub non_test_functions: usize,
    pub upstream_files: usize,
    pub upstream_links: usize,
    pub upstream_unparsed: usize,
    pub cited_functions: usize,
    pub citations: usize,
    pub history_files: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `crates` | `usize` |  |
| `targets` | `usize` |  |
| `modules` | `usize` |  |
| `functions` | `usize` |  |
| `calls` | `usize` | Distinct caller-callee edges. |
| `call_sites` | `usize` |  |
| `unresolved` | `usize` |  |
| `unresolved_by_kind` | `std::collections::BTreeMap<String, usize>` |  |
| `outside` | `usize` |  |
| `external_calls` | `usize` | Calls into std or third-party dependencies, filtered out. |
| `test_targets` | `usize` | Schema 2: integration-test targets. |
| `test_fns` | `usize` | Schema 2: functions carrying a test attribute. |
| `reached_by_tests` | `usize` | Schema 2: non-test functions reached by at least one test. |
| `reached_by_examples` | `usize` | Schema 2: non-test functions reached by at least one example. |
| `non_test_functions` | `usize` | Schema 2: non-test functions. |
| `upstream_files` | `usize` | Schema 2: modules with a parsed `upstream`. |
| `upstream_links` | `usize` | Schema 2: of those, the ones with a `url`. |
| `upstream_unparsed` | `usize` | Schema 2: modules with `upstream_unparsed`. |
| `cited_functions` | `usize` | Schema 2: functions with at least one citation, and all citations. |
| `citations` | `usize` |  |
| `history_files` | `usize` | Schema 2: modules with history. |

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
    fn clone(self: &Self) -> Totals { /* ... */ }
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
    fn default() -> Totals { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
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
    fn eq(self: &Self, other: &Totals) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `RawCall`

One resolved call site, as the builder finds it.

```rust
pub struct RawCall {
    pub from: String,
    pub to: String,
    pub kind: CallKind,
    pub line: u32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `from` | `String` |  |
| `to` | `String` |  |
| `kind` | `CallKind` |  |
| `line` | `u32` |  |

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
    fn clone(self: &Self) -> RawCall { /* ... */ }
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
    fn eq(self: &Self, other: &RawCall) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `function_ids`

Unique ids for the functions of one file, given each one's
`code-walk` qualified name (`name` or `Type::name`) in source order.

```rust
pub fn function_ids(file: &str, quals: &[String]) -> Vec<(String, bool)> { /* ... */ }
```

### Constants and Statics

#### Constant `SCHEMA_VERSION`

Version of the JSON layout; bumped on any breaking change.
Schema 2 (2026-10-06, #746) only adds fields; a schema-1 reader that
ignores unknown fields reads it unchanged. Schema 3 (2026-10-07, #757)
adds the `operator` call kind and `generator`; schema-2 documents still
load ([`OLDEST_READABLE_SCHEMA`]).

```rust
pub const SCHEMA_VERSION: u32 = 3;
```

#### Constant `OLDEST_READABLE_SCHEMA`

The oldest schema this model still reads: every version since only added
fields and values.

```rust
pub const OLDEST_READABLE_SCHEMA: u32 = 1;
```

## Module `code_map`

The code map of a Cargo workspace (GitHub #734): model, layout and SVG.
Moved here from `kovan::code_map` on 2026-10-06; `kovan` re-exports it
and keeps the `cargo metadata` call.
The **code map** of a Cargo workspace: every member crate placed by the
`[package.metadata.kovan]` tag in its `Cargo.toml`, with its required
dependency edges (GitHub #734, step 2 of #729; the tags are #733).

# What it shows

The maintainer's layout, decided 2026-10-06 on #729:

```text
                               outram-park                        (root card)
 row 4   [dhoby-ghaut] [digital-twin-engine] [dover] [pasir-ris]  (each app its own box)
         NEUTRONICS | THERMAL-HYDRAULICS | ... | RISK              ┌ KNOWLEDGE MGMT ┐
 row 3   crates     | ...   (topic boxes are columns, rows 3-2)    │ kovan family   │
 row 2   crates     | ...   highest fidelity left, lowest right    │ by their rows  │
 rows 1-0 ── shared utilities base ──                              └────────────────┘
```

A card carries the crate's name, maturity (0 concept .. 4 human V&V, the
crate's lowest part) and fidelity (0 lumped .. 4 brute force, or a range).
The meaning of every tag is documented, and checked against the
dependency graph, in `crates/kovan/tests/code_map_tags.rs`.

# Where the data comes from: `cargo metadata`, not rust-analyzer

`CodeMap::from_cargo_metadata` reads the JSON of
`cargo metadata --format-version 1 --no-deps`: the package list, each
package's tag and its declared dependencies. Nothing is inferred from
source code and rust-analyzer is never run, so the map is cheap to build
on a desktop or in CI and is shipped to the web as static data (the SVG and
JSON `kovan-cli code-map` writes). Phones only ever render it.

# Edges

Only **required normal** dependencies between workspace members: not
optional ones, not dev- or build-dependencies. That is what a crate needs
in order to build, the same rule the row check uses.

# Determinism

Same `Cargo.toml`s, same map: crates are sorted by name, edges by
`(from, to)`, and the layout ([`layout::layout`]) uses only sorted
vectors and `BTreeMap`s, never a `HashMap`'s iteration order. The SVG
([`svg::render`]) is plain string building with fixed number formatting,
so the same input gives byte-identical output (tested).

Plain `serde` + `std` only, no GUI and no I/O. **Moved 2026-10-06** from
`kovan::code_map` into this wasm-clean crate so web-kovan (`kovan-web`,
GitHub #736) can draw the same map in the browser; `kovan::code_map`
re-exports everything here and keeps the one function that does I/O,
`run_cargo_metadata` (and `load_workspace` on top of it).

```rust
pub mod code_map { /* ... */ }
```

### Modules

## Module `layout`

Where everything on the code map goes, in world units (points at zoom 1,
y down). Pure and deterministic: the same [`CodeMap`] always gives the
same [`Layout`], to the last bit.

# The rules

- **Root** card centred at the top; under it one **app box** per row-4
  crate, side by side, centred over the pyramid.
- **Topic boxes** ([`Topic::COLUMNS`]) are columns through the
  topic rows (3 and 2, plus any other row a topic crate declares). Every
  topic gets a box, even an empty one. **Since 2026-10-06** (maintainer,
  #734) the eight boxes wrap onto **bands of [`BOXES_PER_BAND`]** (two
  rows of four) instead of one row of eight, which made the map about
  7:1 wide. Order is kept (left to right, then the next band). Each box
  labels its own rows in a strip at its left ([`ROW_TAG`]); within a
  band a row's cards line up across the boxes.
- Inside a topic box, **fidelity columns** run from the highest level on
  the left to the lowest on the right. The columns are the levels a crate
  in the box sits at, plus both ends of every range. A column is as wide
  as the most crates any one row puts at that level (ties sit side by
  side, sorted by name, at most [`MAX_TIES`] abreast before wrapping to
  a lane below); a column only a range touches is half a card.
- A **range** crate spans from the left of its `hi` column to the right of
  its `lo` column, on its own lane below the single-level crates of its
  row (lanes assigned greedily, widest range first, so ranges that do not
  overlap share one). `raffles` `[0, 4]` therefore spans the whole Risk box.
- Rows line up across the boxes of a band: a row is as tall as the most
  lanes any box of that band needs in it.
- The **utilities base** (rows 1, then 0) spans the pyramid's width below
  the topic boxes, each row's crates centred.
- The **knowledge-management box** stands to the right, from the app band
  to the bottom of the base; each of its crates sits in its own row's band.

```rust
pub mod layout { /* ... */ }
```

### Types

#### Struct `Rect`

An axis-aligned rectangle: top-left corner and size.

```rust
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` |  |
| `y` | `f64` |  |
| `w` | `f64` |  |
| `h` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn right(self: &Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn bottom(self: &Self) -> f64 { /* ... */ }
  ```

- ```rust
  pub fn centre(self: &Self) -> (f64, f64) { /* ... */ }
  ```

- ```rust
  pub fn overlaps(self: &Self, o: &Rect) -> bool { /* ... */ }
  ```
  Whether the two overlap with positive area.

- ```rust
  pub fn contains(self: &Self, o: &Rect) -> bool { /* ... */ }
  ```
  Whether `o` lies inside this rectangle.

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
    fn clone(self: &Self) -> Rect { /* ... */ }
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
    fn eq(self: &Self, other: &Rect) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `FrameKind`

**Attributes:**

- `Other("#[serde(rename_all = \"kebab-case\")]")`

What kind of box a [`Frame`] is.

```rust
pub enum FrameKind {
    App,
    Topic,
    Utilities,
    KnowledgeManagement,
}
```

##### Variants

###### `App`

###### `Topic`

###### `Utilities`

###### `KnowledgeManagement`

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
    fn clone(self: &Self) -> FrameKind { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &FrameKind) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Frame`

A box on the map.

```rust
pub struct Frame {
    pub kind: FrameKind,
    pub topic: super::Topic,
    pub title: String,
    pub rect: Rect,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `kind` | `FrameKind` |  |
| `topic` | `super::Topic` |  |
| `title` | `String` |  |
| `rect` | `Rect` |  |

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
    fn clone(self: &Self) -> Frame { /* ... */ }
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
    fn eq(self: &Self, other: &Frame) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Card`

A crate's card.

```rust
pub struct Card {
    pub name: String,
    pub rect: Rect,
    pub frame: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` |  |
| `rect` | `Rect` |  |
| `frame` | `usize` | Index into [`Layout::frames`] of the box the card is in. |

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
    fn clone(self: &Self) -> Card { /* ... */ }
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
    fn eq(self: &Self, other: &Card) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Label`

A text label: a row number in the gutter, or a fidelity column's level
in a topic box's title strip. `(x, y)` is the text's centre.

```rust
pub struct Label {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub tip: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` |  |
| `y` | `f64` |  |
| `text` | `String` |  |
| `tip` | `String` | What the label means, shown on hover ([`row_meaning`],<br>[`fidelity_meaning`]). |

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
    fn clone(self: &Self) -> Label { /* ... */ }
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
    fn eq(self: &Self, other: &Label) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Layout`

Everything the map draws, world units.

```rust
pub struct Layout {
    pub root: Rect,
    pub frames: Vec<Frame>,
    pub cards: Vec<Card>,
    pub labels: Vec<Label>,
    pub bounds: Rect,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `root` | `Rect` |  |
| `frames` | `Vec<Frame>` |  |
| `cards` | `Vec<Card>` | One per crate, in [`CodeMap::crates`] order. |
| `labels` | `Vec<Label>` |  |
| `bounds` | `Rect` | The extent of everything above. |

##### Implementations

###### Methods

- ```rust
  pub fn card(self: &Self, name: &str) -> Option<&Card> { /* ... */ }
  ```
  The card of the crate called `name`.

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
    fn clone(self: &Self) -> Layout { /* ... */ }
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
    fn eq(self: &Self, other: &Layout) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
### Functions

#### Function `row_meaning`

What a row means (hover text of the row labels; the tag documentation in
`crates/kovan/tests/code_map_tags.rs`).

```rust
pub fn row_meaning(row: u8) -> &'static str { /* ... */ }
```

#### Function `fidelity_meaning`

What a fidelity column means (hover text of the F labels).

```rust
pub fn fidelity_meaning(level: i8) -> &'static str { /* ... */ }
```

#### Function `layout`

Lay out `map`. See the module docs for the rules.

```rust
pub fn layout(map: &super::CodeMap) -> Layout { /* ... */ }
```

#### Function `check`

Every rule a layout must obey, as a list of problems (empty when all
hold): each crate placed exactly once, inside its box; no two cards
overlap; nothing overlaps the root; within a topic box, a crate of
higher fidelity never starts to the right of one of lower fidelity; and
a crate of row r sits no lower on the page than one of row r - 1 in the
same box. Public so the real-workspace test can run it too.

```rust
pub fn check(map: &super::CodeMap, l: &Layout) -> Vec<String> { /* ... */ }
```

### Constants and Statics

#### Constant `CARD_W`

A card, world units.

```rust
pub const CARD_W: f64 = 230.0;
```

#### Constant `CARD_H`

```rust
pub const CARD_H: f64 = 56.0;
```

#### Constant `ROOT_W`

The root card.

```rust
pub const ROOT_W: f64 = 300.0;
```

#### Constant `ROOT_H`

```rust
pub const ROOT_H: f64 = 72.0;
```

#### Constant `GAP`

Space between cards, and between lanes.

```rust
pub const GAP: f64 = 14.0;
```

#### Constant `PAD`

Inner padding of a box.

```rust
pub const PAD: f64 = 16.0;
```

#### Constant `HEADER`

Height of a box's title strip (title and fidelity labels).

```rust
pub const HEADER: f64 = 46.0;
```

#### Constant `BOX_GAP`

Space between boxes.

```rust
pub const BOX_GAP: f64 = 30.0;
```

#### Constant `BAND_GAP`

Space between bands of rows (row 3 to row 2 etc.).

```rust
pub const BAND_GAP: f64 = 26.0;
```

#### Constant `GUTTER`

Width of the row-label gutter left of the pyramid.

```rust
pub const GUTTER: f64 = 70.0;
```

#### Constant `MAX_TIES`

Most crates of one fidelity level side by side in one row; more wrap
onto a further lane below. A drawing choice (2026-10-06), not one of the
maintainer's rules: without it the six `outram-foam-*` crates at
fidelity 3 made the map about seven times wider than tall.

```rust
pub const MAX_TIES: usize = 3;
```

#### Constant `BOXES_PER_BAND`

Topic boxes per band (maintainer, 2026-10-06: two rows of four).

```rust
pub const BOXES_PER_BAND: usize = 4;
```

#### Constant `ROW_TAG`

Width of the strip inside a topic box's left edge that labels its rows.

```rust
pub const ROW_TAG: f64 = 30.0;
```

## Module `svg`

The code map as a standalone SVG, for the static site (GitHub #734).

Plain string building from [`super::layout::Layout`]: no GUI library and
no new dependency. Every number is printed with one decimal, nothing
depends on time, locale or iteration order, so the same [`CodeMap`] gives
byte-identical output (tested).

What it draws, back to front: the boxes and their titles, the row and
fidelity labels, the required dependency edges (faint curves, the same
edge-to-edge Bézier as the mind map's connectors,
[`crate::mindmap_view::connector_sized`]), then the root and one card per
crate. A card shows the crate's name, its maturity (a numbered badge and
its label, `"2 · parts at 3"` when modules are rated higher) and its
fidelity; maturity-0 crates are greyed and dashed. Each card is a
`<g class="card" data-crate="…">` with a `<title>` tooltip (row, topic,
fidelity, maturity, the higher-rated modules with their reasons, the
description), and each edge a `<path class="edge" data-from data-to>`, so
a page can highlight a crate's edges with a few lines of script.

Colours are CSS classes in the SVG's own `<style>`, with a dark variant
under `prefers-color-scheme: dark`, so the file reads in either theme
whether it is inlined or shown as an `<img>`.

```rust
pub mod svg { /* ... */ }
```

### Functions

#### Function `escape`

Escape text for XML content and attribute values.

```rust
pub fn escape(s: &str) -> String { /* ... */ }
```

#### Function `topic_colour`

The topic's colour (the card's left strip and its box's border), as hex.

```rust
pub fn topic_colour(t: super::Topic) -> &'static str { /* ... */ }
```

#### Function `maturity_colour`

The maturity badge's colour, 0 (grey) to 4 (deep green).

```rust
pub fn maturity_colour(level: u8) -> &'static str { /* ... */ }
```

#### Function `card_subtitle`

The card's second line: `"M2 AI V&V · F3"`.

```rust
pub fn card_subtitle(c: &super::CrateNode) -> String { /* ... */ }
```

#### Function `tooltip`

The hover text: row, topic, fidelity, maturity, higher-rated modules and
the description, one per line.

```rust
pub fn tooltip(map: &super::CodeMap, c: &super::CrateNode) -> String { /* ... */ }
```

#### Function `render`

The SVG document for `map` laid out as `layout`.

```rust
pub fn render(map: &super::CodeMap, layout: &super::layout::Layout) -> String { /* ... */ }
```

### Types

#### Enum `Topic`

**Attributes:**

- `Other("#[serde(rename_all = \"kebab-case\")]")`

The box a crate is drawn in (`topic` in the tag).

```rust
pub enum Topic {
    App,
    Neutronics,
    ThermalHydraulics,
    FuelPerformance,
    StructuralMechanics,
    Chemistry,
    FuelCycle,
    GranularDem,
    Risk,
    Utility,
    KnowledgeManagement,
}
```

##### Variants

###### `App`

###### `Neutronics`

###### `ThermalHydraulics`

###### `FuelPerformance`

###### `StructuralMechanics`

###### `Chemistry`

###### `FuelCycle`

###### `GranularDem`

###### `Risk`

###### `Utility`

###### `KnowledgeManagement`

##### Implementations

###### Methods

- ```rust
  pub fn as_str(self: Self) -> &'static str { /* ... */ }
  ```
  The tag's spelling, e.g. `"thermal-hydraulics"`.

- ```rust
  pub fn parse(s: &str) -> Option<Topic> { /* ... */ }
  ```
  Parse the tag's spelling.

- ```rust
  pub fn title(self: Self) -> &'static str { /* ... */ }
  ```
  The box title drawn on the map.

- ```rust
  pub fn has_no_fidelity(self: Self) -> bool { /* ... */ }
  ```
  Whether a crate of this topic carries no fidelity (utilities and

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
    fn clone(self: &Self) -> Topic { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Topic) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Topic) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Topic) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `Fidelity`

**Attributes:**

- `Other("#[serde(untagged)]")`

A crate's fidelity on the absolute 0..=4 scale (maintainer,
2026-10-06): 0 lumped, 1 one resolved dimension, 2 two (subchannel),
3 three (CFD, diffusion), 4 brute force (Monte Carlo, first-principles
data). Serialised as the tag spells it: `3` or `[1, 3]`.

```rust
pub enum Fidelity {
    Level(u8),
    Range(u8, u8),
}
```

##### Variants

###### `Level`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u8` |  |

###### `Range`

`(lo, hi)`, `lo < hi`: the crate spans these levels.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `u8` |  |
| 1 | `u8` |  |

##### Implementations

###### Methods

- ```rust
  pub fn hi(self: Self) -> u8 { /* ... */ }
  ```
  Highest level the crate reaches.

- ```rust
  pub fn lo(self: Self) -> u8 { /* ... */ }
  ```
  Lowest level the crate reaches.

- ```rust
  pub fn label(self: Self) -> String { /* ... */ }
  ```
  Short label, `"F3"` or `"F1–3"`.

- ```rust
  pub fn level_meaning(level: u8) -> &'static str { /* ... */ }
  ```
  What a level means, from the tag documentation.

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
    fn clone(self: &Self) -> Fidelity { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &Fidelity) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `MaturityModule`

A module rated above its crate (`[[package.metadata.kovan.maturity_modules]]`).

```rust
pub struct MaturityModule {
    pub module: String,
    pub level: u8,
    pub why: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `module` | `String` | Path from the library root, `a::b`. |
| `level` | `u8` |  |
| `why` | `String` |  |

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
    fn clone(self: &Self) -> MaturityModule { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &MaturityModule) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `CrateNode`

One workspace member and its tag.

```rust
pub struct CrateNode {
    pub name: String,
    pub description: Option<String>,
    pub row: u8,
    pub topic: Topic,
    pub fidelity: Option<Fidelity>,
    pub maturity: u8,
    pub maturity_modules: Vec<MaturityModule>,
    pub lib_dir: Option<String>,
    pub dir: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` |  |
| `description` | `Option<String>` | The `description` from `Cargo.toml`, if any. |
| `row` | `u8` | Degree of integration: 0-1 utilities, 2 domain solvers, 3 coupled<br>multiphysics, 4 integrated GUI apps. |
| `topic` | `Topic` |  |
| `fidelity` | `Option<Fidelity>` |  |
| `maturity` | `u8` | The crate's lowest part, 0..=4. |
| `maturity_modules` | `Vec<MaturityModule>` |  |
| `lib_dir` | `Option<String>` | Directory of the library's root source file, for checking that<br>`maturity_modules` exist. Machine-specific, so never serialised. |
| `dir` | `Option<String>` | The crate's folder relative to the workspace root, `/`-separated<br>(e.g. `crates/kovan`), for links into the repository. |

##### Implementations

###### Methods

- ```rust
  pub fn maturity_summary(self: &Self) -> String { /* ... */ }
  ```
  `"2 · parts at 4"` when modules are rated higher, else `"2"`.

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
    fn clone(self: &Self) -> CrateNode { /* ... */ }
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
    fn eq(self: &Self, other: &CrateNode) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `Edge`

`from` needs `to` to build (a required normal dependency).

```rust
pub struct Edge {
    pub from: String,
    pub to: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `from` | `String` |  |
| `to` | `String` |  |

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
    fn clone(self: &Self) -> Edge { /* ... */ }
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
- **Eq**
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Edge) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Edge) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Edge) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `CodeMap`

The whole map's data: crates sorted by name, edges sorted.

```rust
pub struct CodeMap {
    pub root: String,
    pub crates: Vec<CrateNode>,
    pub edges: Vec<Edge>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `root` | `String` |  |
| `crates` | `Vec<CrateNode>` |  |
| `edges` | `Vec<Edge>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn from_cargo_metadata(json: &str) -> Result<CodeMap, Vec<String>> { /* ... */ }
  ```
  Build the map from `cargo metadata --format-version 1 --no-deps`

- ```rust
  pub fn get(self: &Self, name: &str) -> Option<&CrateNode> { /* ... */ }
  ```
  The crate called `name`.

- ```rust
  pub fn dependencies(self: &Self, name: &str) -> Vec<&str> { /* ... */ }
  ```
  What `name` depends on (required), sorted.

- ```rust
  pub fn dependents(self: &Self, name: &str) -> Vec<&str> { /* ... */ }
  ```
  What depends on `name` (required), sorted.

- ```rust
  pub fn placement_problems(self: &Self) -> Vec<String> { /* ... */ }
  ```
  Placement rules the tags must obey, against each other and the

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
    fn clone(self: &Self) -> CodeMap { /* ... */ }
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
    fn eq(self: &Self, other: &CodeMap) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
### Functions

#### Function `maturity_label`

The maturity rung's short label (#729, maintainer 2026-10-06).

```rust
pub fn maturity_label(level: u8) -> &'static str { /* ... */ }
```

#### Function `label_width`

Approximate width, in px, of `s` set in a bold sans-serif at `size` px
(0.62 em per character, a deliberate over-estimate for crate names).

```rust
pub fn label_width(s: &str, size: f64) -> f64 { /* ... */ }
```

#### Function `fit_label`

Fit `s` into `avail` px: the largest font size from `max` down to `min`
that fits, else `min` and the text cut with an ellipsis. The full text
stays in the tooltip and the detail panel. Used by the SVG and the GUI
so both cut the same names.

```rust
pub fn fit_label(s: &str, avail: f64, max: f64, min: f64) -> (String, f64) { /* ... */ }
```

### Constants and Statics

#### Constant `ROOT_TITLE`

The title of the root card at the top of the map.

```rust
pub const ROOT_TITLE: &str = "outram-park";
```

## Module `geometry`

World-space [`geometry::Point`] and [`geometry::Bounds`].
World-space geometry shared by kovan's map views: a [`Point`] and an
axis-aligned [`Bounds`]. Moved 2026-10-06 from `kovan::mindmap_layout`
(which re-exports both) so the wasm-clean views (`mindmap_view`,
`code_map`, web-kovan) need nothing from the AGPL `kovan` crate.

```rust
pub mod geometry { /* ... */ }
```

### Types

#### Struct `Point`

A point in world space (the same space the mind-map and code-map layouts place cards in).

```rust
pub struct Point {
    pub x: f64,
    pub y: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `x` | `f64` |  |
| `y` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new(x: f64, y: f64) -> Self { /* ... */ }
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

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Point { /* ... */ }
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
    fn eq(self: &Self, other: &Point) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `Bounds`

An axis-aligned bounding box in world space, as produced by
`kovan::mindmap_layout::bounds_for`.

```rust
pub struct Bounds {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `min_x` | `f64` |  |
| `min_y` | `f64` |  |
| `max_x` | `f64` |  |
| `max_y` | `f64` |  |

##### Implementations

###### Methods

- ```rust
  pub fn width(self: &Self) -> f64 { /* ... */ }
  ```
  Width, floored at `1.0` so a single-point or degenerate bounds

- ```rust
  pub fn height(self: &Self) -> f64 { /* ... */ }
  ```
  Height, floored at `1.0` — see [`width`](Self::width).

- ```rust
  pub fn centre(self: &Self) -> Point { /* ... */ }
  ```
  The bounds' centre point.

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
    fn clone(self: &Self) -> Bounds { /* ... */ }
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
    fn eq(self: &Self, other: &Bounds) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
## Module `fuzzy`

The fuzzy scorer of kovan's finders, moved here from `kovan::fuzzy` on
2026-10-06 for web-kovan's search bar.
Fuzzy matching for Kovan's finders (the PDF reader's literature finder,
and since 2026-10-06 web-kovan's search bar; moved here from `kovan` that
day so the wasm page can use it, `kovan::fuzzy` re-exports it).

Ported from `fuzzy_score` in
`crates/njoy-outram-park-fork/src/bin/njoy-tui/nuclides.rs` (the njoy TUI's
nuclide finder), which lives in another crate's binary and so cannot be
depended on. Same algorithm, same constants; keep the two in step. Hand
rolled, as there, rather than a new fuzzy-matching dependency.

```rust
pub mod fuzzy { /* ... */ }
```

### Functions

#### Function `fuzzy_score`

Score how well `query` matches `candidate`, or `None` if it does not.
Both are lowercased here, so the match is case-insensitive.

- **Substring** matches score highest (1000+), with a bonus for a prefix
  match and for a tighter candidate.
- **Subsequence** matches (every query character in order, fzf-style)
  score below any substring, penalised by how spread out they are.

```rust
pub fn fuzzy_score(query: &str, candidate: &str) -> Option<i32> { /* ... */ }
```

## Module `mindmap_view`

The star (ring) layout and the scrollable-canvas arithmetic of kovan's
map views. Moved here from `kovan::mindmap_view` on 2026-10-06.
Geometry of the Mindmap page's star view and its pan/zoom viewport
(GitHub issue #243, epic #241). GUI-free, so it is tested headlessly and
builds everywhere this crate does.

What belongs here: where each card of the drill-in star goes
([`star_positions`]), the star's extent ([`star_bounds`]), and the
arithmetic of the scrollable canvas it is drawn on ([`CanvasLayout`]):
how big the canvas is at a zoom, where a world point lands on it, and the
**pan limit**: 25 % of the map's own size past each edge (maintainer
direction, 2026-09-22), plus half a viewport width sideways (added the same
day, for more horizontal room; see [`HORIZONTAL_PAN_VIEWPORT_FRACTION`]).

What does not belong here: drawing, input, or which nodes exist. The egui
page is `kovan::mindmap`; the node set comes from the knowledge index.

# Why not `kovan::mindmap_layout::Camera`

That camera maps world space straight onto the viewport, with no scroll
bars and nothing that limits panning. The page instead draws on an
`egui::ScrollArea` canvas, which gives both, with the same approach as the
`htgr_sim_v1` v1.1 plant view in `outram-park-digital-twin-engine`
(`examples/htgr_sim_v1/app/plant_v1_1.rs`, `content_layout`). The logic is
ported, not shared: that crate is not a dependency of this one. The
difference is the margin: there it is 40 % of the viewport, here 25 % of
the content plus, horizontally, half the viewport.

# Units

World units are points at zoom 1. A card is [`CARD_SIZE`] world units; at
zoom `z` it is drawn `z` times that size, text included.

```rust
pub mod mindmap_view { /* ... */ }
```

### Types

#### Struct `StarLayout`

Where a whole star goes when some ring cards are expanded in place
(GitHub issue #246): `ring[i]` is ring card `i`, and `fans[i]` holds the
positions of its sub-concepts (empty unless card `i` is expanded).

```rust
pub struct StarLayout {
    pub ring: Vec<crate::geometry::Point>,
    pub fans: Vec<Vec<crate::geometry::Point>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `ring` | `Vec<crate::geometry::Point>` |  |
| `fans` | `Vec<Vec<crate::geometry::Point>>` |  |

##### Implementations

###### Methods

- ```rust
  pub fn all_points(self: &Self) -> impl Iterator<Item = Point> + ''_ { /* ... */ }
  ```
  Every card centre in the layout: ring, then each fan in order.

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
    fn clone(self: &Self) -> StarLayout { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &StarLayout) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `CanvasLayout`

The scrollable canvas the star is drawn on, at one zoom and viewport size.

The canvas is the map at `zoom`, plus a margin of [`PAN_MARGIN_FRACTION`]
of the drawn map's width and height on every side, so the scroll area lets
the view travel exactly that far past each edge and no further.
Horizontally the margin also gains [`HORIZONTAL_PAN_VIEWPORT_FRACTION`] of
the viewport width. When the canvas would still be smaller than the
viewport on an axis, it is widened to the viewport and the map is centred
on that axis (nothing to scroll there).

```rust
pub struct CanvasLayout {
    pub origin: (f64, f64),
    pub size: (f64, f64),
    pub zoom: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `origin` | `(f64, f64)` | Where the world origin lands on the canvas, points from its top left. |
| `size` | `(f64, f64)` | Canvas size, points. |
| `zoom` | `f64` | World-to-canvas scale. |

##### Implementations

###### Methods

- ```rust
  pub fn new(bounds: Bounds, zoom: f64, viewport: (f64, f64)) -> Self { /* ... */ }
  ```
  Lay out `bounds` at `zoom` in a viewport of `viewport` points.

- ```rust
  pub fn to_canvas(self: &Self, p: Point) -> (f64, f64) { /* ... */ }
  ```
  Canvas position, points, of world point `p`.

- ```rust
  pub fn to_world(self: &Self, c: (f64, f64)) -> Point { /* ... */ }
  ```
  World point at canvas position `c`: the inverse of [`Self::to_canvas`].

- ```rust
  pub fn offset_centring(self: &Self, p: Point, viewport: (f64, f64)) -> (f64, f64) { /* ... */ }
  ```
  The scroll offset that puts world point `p` at the middle of a

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
    fn clone(self: &Self) -> CanvasLayout { /* ... */ }
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
    fn eq(self: &Self, other: &CanvasLayout) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `star_positions`

Where the `n` cards around the centre of the star go, in world units, with
the centre card (the concept you are on) at the origin.

The cards sit evenly on one ring, the first at the top and the rest
clockwise. The ring is as small as it can be without two cards touching
(checked in `no_two_cards_overlap`):

- neighbours on the ring are a chord `2 r sin(pi / n)` apart, which must be
  at least a card diagonal plus [`CARD_GAP`];
- every ring card must also clear the centre card, so `r` is at least one
  diagonal plus the gap.

Measuring by the diagonal is conservative (cards are wider than tall), but
it holds in every direction, so there is no angle at which two cards meet.

```rust
pub fn star_positions(n: usize) -> Vec<crate::geometry::Point> { /* ... */ }
```

#### Function `star_layout`

Lay out a star whose ring card `i` has `fan_sizes[i]` sub-concepts shown
(zero for a collapsed card), with no two cards overlapping.

The ring starts at the radius [`star_positions`] would use. Expanded fans
can reach into their neighbours, so while any two cards (centre, ring or
fan) are closer than a card plus [`CARD_GAP`], the ring radius grows by
10 % and everything is placed again. This always ends: neighbouring ring
cards separate in proportion to the radius while each fan keeps its size.
The loop is capped at [`MAX_RING_GROWTH_STEPS`] as a guard, never reached
in the tested range.

```rust
pub fn star_layout(fan_sizes: &[usize]) -> StarLayout { /* ... */ }
```

#### Function `star_layout_with_parent`

[`star_layout`], rotated half a step when the "up one level" card is drawn
above the star (GitHub issue #283).

[`star_positions`] puts ring card 0 exactly straight up, which is where
the dotted connector to the parent card and the Up button on it go — the
card would cover both. Turning the ring by half a step (`pi / n`) puts the
*gap* between two ring cards at the top instead, for every `n`.

~~Radii are untouched, so no card moves relative to any other.~~
**CORRECTED 2026-09-23** — a rotated star can come out with a *larger*
ring: [`cards_collide`] compares axis-aligned boxes, which is not
rotation-invariant (cards are 170 x 46, so two of them side by side need
far more room than two stacked), so the growth loop can fire for the
turned ring where it did not for the straight one. Seen at `n = 10`. The
invariant that holds is the one that matters: no two cards overlap, and no
card sits in the corridor straight up.

`has_parent = false` is exactly [`star_layout`].

```rust
pub fn star_layout_with_parent(fan_sizes: &[usize], has_parent: bool) -> StarLayout { /* ... */ }
```

#### Function `parent_position`

Where the "up one level" card goes: straight above the star, one card
spacing clear of the furthest card in `layout` (#283).

Measuring from the **furthest** card, not the ring radius, is what keeps
it clear of an expanded fan that reaches further out than the ring does.
One spacing (a card diagonal plus [`CARD_GAP`]) is enough on its own: any
card near enough on the x axis to matter lies at most `furthest` from the
centre, so it is at least a spacing below this card on the y axis, and
[`cards_collide`] needs both axes to be close.

```rust
pub fn parent_position(layout: &StarLayout) -> crate::geometry::Point { /* ... */ }
```

#### Function `up_button_centre`

Where the Up button's box sits on the dotted run between the centre card's
top edge and the parent card's bottom edge (#283).

The middle of that run, **but never nearer the star than the furthest card
reaches**. Half-way is the obvious place and is where it ends up when
there is no ring at all, but an expanded fan can swing a card up to within
a few points of the corridor — one was found sitting exactly on the button
at `n = 3` — and a button drawn under a card cannot be pressed. Pushing it
past the furthest card's radius clears *every* card whatever its angle:
each one has `|y| <= furthest`, so the gap on the y axis alone is then
more than half the two boxes' heights, and [`cards_collide`]-style overlap
needs both axes to be close.

```rust
pub fn up_button_centre(layout: &StarLayout) -> crate::geometry::Point { /* ... */ }
```

#### Function `connector`

A connector between two cards: a cubic Bézier curve `[start, control 1,
control 2, end]` in world units, from an edge of one card to an edge of
the other (maintainer direction, 2026-09-22: "a smooth curved line from
edge to edge, not to the centre of the box").

**Which edges** depends on where the cards sit relative to each other:
the connector runs along the axis with the larger clear gap between them.
Side by side, it leaves the facing left/right edges at their midpoints;
one above the other, the facing top/bottom edges. It leaves and arrives at
right angles to those edges, the control points pulled out along the same
axis by half the distance between the ends (at least [`CARD_GAP`]), which
is what makes it read as one smooth S or arc.

`None` when the cards overlap on both axes (possible with pinned cards):
there is no clear edge to join, and a curve through the cards would be
noise. A drawing rule, nothing physical.

```rust
pub fn connector(a: crate::geometry::Point, b: crate::geometry::Point) -> Option<[crate::geometry::Point; 4]> { /* ... */ }
```

#### Function `connector_from_centre`

[`connector`] from the centre card, at the origin and sized
[`CENTRE_CARD_SIZE`], to an ordinary card at `b`: the curve leaves the
larger card's own edge, so it never shows through the card.

```rust
pub fn connector_from_centre(b: crate::geometry::Point) -> Option<[crate::geometry::Point; 4]> { /* ... */ }
```

#### Function `connector_sized`

[`connector`] between a card of size `size_a` at `a` and one of size
`size_b` at `b` (centres and full sizes, world units). Public since
2026-10-06 so the code map ([`crate::code_map`]) draws its dependency
edges with the same curve.

```rust
pub fn connector_sized(a: crate::geometry::Point, size_a: (f64, f64), b: crate::geometry::Point, size_b: (f64, f64)) -> Option<[crate::geometry::Point; 4]> { /* ... */ }
```

#### Function `star_bounds`

The world-space box that holds every card: the centre card at the origin
(when `has_centre`) and a card at each of `ring`.

```rust
pub fn star_bounds(has_centre: bool, ring: &[crate::geometry::Point]) -> crate::geometry::Bounds { /* ... */ }
```

#### Function `fit_zoom`

The zoom at which `bounds` fits inside `viewport`, margins excluded.

```rust
pub fn fit_zoom(bounds: crate::geometry::Bounds, viewport: (f64, f64)) -> f64 { /* ... */ }
```

### Constants and Statics

#### Constant `CARD_SIZE`

A card's size in world units (points at zoom 1). A drawing choice.

```rust
pub const CARD_SIZE: (f64, f64) = _;
```

#### Constant `CENTRE_CARD_SIZE`

The centre card's size, world units: larger than a ring card, with larger
text, so the concept you are on stands out (maintainer direction,
2026-10-06: "the central node needs to be bigger in font size"). Its title
wraps onto two lines. A drawing choice.

```rust
pub const CENTRE_CARD_SIZE: (f64, f64) = _;
```

#### Constant `CARD_GAP`

Least clear space between two cards, world units. A drawing choice.

```rust
pub const CARD_GAP: f64 = 24.0;
```

#### Constant `PAN_MARGIN_FRACTION`

How far past each edge of the map the view can pan, as a fraction of the
map's own width and height (maintainer direction, 2026-09-22: "don't let
me scroll past 25% of where the mindmap content is").

```rust
pub const PAN_MARGIN_FRACTION: f64 = 0.25;
```

#### Constant `HORIZONTAL_PAN_VIEWPORT_FRACTION`

Extra horizontal pan room, as a fraction of the viewport width, added on
each side on top of [`PAN_MARGIN_FRACTION`] (maintainer direction,
2026-09-22: "give me more horizontal space to pan around"). Half a
viewport lets either side edge of the map be brought to the middle of the
screen, even at Fit, where the map is usually narrower than the window and
the content margin alone left nothing to scroll sideways. Vertical pan
room is unchanged. A viewing margin, nothing physical.

```rust
pub const HORIZONTAL_PAN_VIEWPORT_FRACTION: f64 = 0.5;
```

#### Constant `UP_BUTTON_SIZE`

The Up button's box, in world units — the box drawn **on** the dotted
connector between the centre card and the parent card (#283). A drawing
choice, sized to hold one arrow glyph.

```rust
pub const UP_BUTTON_SIZE: (f64, f64) = _;
```

#### Constant `MAX_RING_GROWTH_STEPS`

Guard on [`star_layout`]'s ring growth: 1.1^200 is about 2e8, far past any
radius a real star needs.

```rust
pub const MAX_RING_GROWTH_STEPS: usize = 200;
```

## Module `anchoring`

Hypothesis-style robust annotation anchoring (W3C selectors, fuzzy re-anchoring; GitHub #754).
# Robust annotation anchoring (GitHub #754)

Attach a note to a span of text so that it **survives edits to that
text**, the way Hypothesis keeps web and PDF annotations attached. An
annotation stores several [`Selector`]s in the W3C Web Annotation shape:

- [`TextQuoteSelector`]: the exact text plus 32 characters either side;
- [`TextPositionSelector`]: `char` offsets;
- [`PageSelector`]: the page, for paged text (Hypothesis's structural
  selector for PDFs).

**Describing** ([`describe`], [`describe_in_pages`]) makes them from a
text and a range. **Anchoring** ([`anchor`], [`anchor_in_pages`]) finds
the range again in a possibly-changed text, trying, in upstream's order:

1. the position, accepted only if the text there still equals the quote
   ([`AnchorStrategy::Position`]);
2. exact occurrences of the quote, ranked by context and nearness to the
   old position ([`AnchorStrategy::ExactQuote`]);
3. the fuzzy (Myers bit-parallel edit-distance) search, at most
   `min(256, quote/2)` edits, ranked the same way
   ([`AnchorStrategy::FuzzyQuote`]).

If none applies the annotation is [`Anchoring::Orphaned`]. Every match
carries upstream's score in [0, 1] (50 quote, 20 prefix, 20 suffix,
2 position, normalised; see [`match_quote`](mod@match_quote)). Upstream applies no
score threshold and neither does this port: callers that want one (for
example "flag for review below 0.75") apply it to [`Anchor::score`].

Paged text ([`anchor_in_pages`]) compares quotes **ignoring whitespace**,
as upstream's PDF path does, since re-extraction moves spaces and line
breaks. Plain text ([`anchor`]) does not, as upstream's HTML path.

Units: all offsets are Unicode scalar values (`char`s), per the W3C
model; Hypothesis itself counts UTF-16 code units ([`selector`](crate::anchoring::selector) doc).

Pure `std` + `serde`; no I/O. Nothing in kovan uses it yet: wiring it
into review stamps, `[[relation]]` anchors or PDF highlights is a
separate maintainer decision (#754).

```rust
pub mod anchoring { /* ... */ }
```

### Modules

## Module `approx_match`

Myers' bit-parallel approximate string matching (GitHub #754).

A line-by-line port of `approx-string-match`, the matcher Hypothesis's
`match-quote.ts` calls. It finds every end position in `text` where
`pattern` matches with the fewest edits (insertions, deletions or
substitutions), up to `max_errors`, and then the start of each match.

References, as upstream cites them:
1. G. Myers, "A Fast Bit-Vector Algorithm for Approximate String Matching
   Based on Dynamic Programming", J. ACM 46(3), 395-415, 1999.
2. M. Šošić, "An SIMD dynamic programming C/C++ library", doctoral
   dissertation, University of Zagreb, 2014.

**One deliberate difference from upstream: the unit of text.** Upstream
counts UTF-16 code units, so a character outside the Basic Multilingual
Plane (an emoji) counts as two. This port counts Unicode scalar values
(`char`), which is what the W3C Web Annotation model specifies for
`TextPositionSelector` and what upstream's own test calls the behaviour
"we probably want". Upstream's `unicode` fixtures are ported with the
expectations adjusted accordingly (see `tests/upstream_approx.rs`).

The word size stays 32 bits, as upstream, so the block arithmetic (and
its edge cases at pattern lengths of 32 and 64) is exercised exactly as
upstream's tests exercise it.

```rust
pub mod approx_match { /* ... */ }
```

### Types

#### Struct `Match`

One approximate match of a pattern in a text, in `char` offsets.

```rust
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub errors: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `start` | `usize` | Start offset of the match in the text. |
| `end` | `usize` | End offset (exclusive) of the match in the text. |
| `errors` | `usize` | Edits (insertions, deletions, substitutions) between the pattern and<br>the matched text. |

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
    fn clone(self: &Self) -> Match { /* ... */ }
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
    fn eq(self: &Self, other: &Match) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `search`

The closest matches of `pattern` in `text`: every match with the lowest
error count, or none if none has `max_errors` or fewer. Upstream's
default export `search`. Offsets are `char` indices.

```rust
pub fn search(text: &[char], pattern: &[char], max_errors: usize) -> Vec<Match> { /* ... */ }
```

#### Function `search_str`

[`search`] on string slices; offsets are still `char` indices.

```rust
pub fn search_str(text: &str, pattern: &str, max_errors: usize) -> Vec<Match> { /* ... */ }
```

## Module `match_quote`

Find the best approximate match of a quote in a text, scored by how well
the quote, its prefix and suffix, and its expected position agree
(Hypothesis's `matchQuote`, GitHub #754).

The weights and the error budget are upstream's, unchanged:

| Term | Weight | Score in [0, 1] |
|---|---|---|
| quote | 50 | `1 - errors / quote.len()` |
| prefix | 20 | similarity of the text before the match to `prefix` |
| suffix | 20 | similarity of the text after the match to `suffix` |
| position | 2 | `1 - |start - hint| / text.len()` (a tie-breaker) |

The total is divided by 92. The candidates are only the matches with the
fewest errors, and at most `min(256, quote.len() / 2)` errors are allowed;
beyond that there is no match. Upstream has **no score threshold**: the
best candidate is returned whatever its score. That is kept; the score is
returned so a caller can apply its own.

```rust
pub mod match_quote { /* ... */ }
```

### Types

#### Struct `QuoteMatch`

The best match of a quote, in `char` offsets of the searched text.

```rust
pub struct QuoteMatch {
    pub start: usize,
    pub end: usize,
    pub errors: usize,
    pub score: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `start` | `usize` | Start offset of the match. |
| `end` | `usize` | End offset (exclusive) of the match. |
| `errors` | `usize` | Edits between the quote and the matched text (0 for an exact match). |
| `score` | `f64` | Upstream's normalised score in [0, 1]; 1.0 is a perfect match of<br>the quote and of both context strings at the expected position.<br>(The position term can go below 0 when the hint lies beyond the end<br>of the text, as upstream.) |

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
    fn clone(self: &Self) -> QuoteMatch { /* ... */ }
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
    fn eq(self: &Self, other: &QuoteMatch) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `QuoteContext`

What the quote was expected to sit in (upstream's `Context`).

```rust
pub struct QuoteContext {
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub hint: Option<usize>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `prefix` | `Option<String>` | Expected text just before the quote. |
| `suffix` | `Option<String>` | Expected text just after the quote. |
| `hint` | `Option<usize>` | Expected start offset (`char`) of the quote in the text. |

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
    fn clone(self: &Self) -> QuoteContext { /* ... */ }
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
    fn default() -> QuoteContext { /* ... */ }
    ```

- **Eq**
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
    fn eq(self: &Self, other: &QuoteContext) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `match_quote`

The best approximate match of `quote` in `text`, or `None` if the quote
is empty or nothing is within upstream's error budget. Offsets are
`char` indices. Upstream `matchQuote`.

```rust
pub fn match_quote(text: &str, quote: &str, context: &QuoteContext) -> Option<QuoteMatch> { /* ... */ }
```

## Module `offsets`

Whitespace-insensitive offset translation, used by the page anchoring to
compare a quote with re-extracted PDF text that differs in its spaces.

**Left out:** upstream's optional Unicode NFKD normalisation inside
`translateOffsets` (which relates an "fi" ligature to "f" + "i"). It
needs a Unicode normalisation table, and no wasm-clean crate for one is
in the root `[workspace.dependencies]`; upstream itself switches it off
on its quote-matching path (`normalize: false`) and uses it only to
relate PDF.js's text API to its rendered text layer, a step kovan does
not have. A ligature difference therefore costs edit errors here instead.

```rust
pub mod offsets { /* ... */ }
```

### Functions

#### Function `is_space`

Upstream's `isSpace`: the ASCII spaces PDF.js produces, plus NBSP. Not
every Unicode space, deliberately (upstream's comment).

```rust
pub fn is_space(c: char) -> bool { /* ... */ }
```

#### Function `is_not_space`

`!is_space(c)`.

```rust
pub fn is_not_space(c: char) -> bool { /* ... */ }
```

#### Function `strip_spaces`

The characters of `s` with [`is_space`] ones removed (upstream
`stripSpaces`).

```rust
pub fn strip_spaces(s: &[char]) -> Vec<char> { /* ... */ }
```

#### Function `translate_offsets`

Translate a `(start, end)` pair of offsets in `input` into the matching
offsets in `output`, two strings that hold the same "important"
characters (those passing `filter`) with different ignored ones between
them. Of several equivalent output offsets, the largest start and the
smallest end are chosen, so leading and trailing ignored characters are
trimmed. Out-of-range inputs are clamped, as upstream. Upstream
`translateOffsets` without its `normalize` option (module doc).

```rust
pub fn translate_offsets<F: Fn(char) -> bool>(input: &[char], output: &[char], start: i64, end: i64, filter: F) -> (usize, usize) { /* ... */ }
```

## Module `pages`

Describe and anchor a range of a paged text: the extracted text of a
PDF, one string per page. Upstream's PDF path without PDF.js: the "page
text" is the string kovan's extractor produced for that page.

As upstream:
- The position selector counts from the start of page 0, over the page
  texts joined with nothing between them.
- Quote matching **ignores whitespace** ([`super::offsets::is_space`]):
  page text, quote, prefix and suffix are compared with their spaces
  stripped, because re-extraction (a different PDF library or version)
  often adds or drops spaces and line breaks.
- Pages are searched in order of distance from the page the position
  points at, and the search stops early on an exact quote with an exact
  prefix or suffix (or an exact quote and no context at all).
- The quote's prefix and suffix are taken from the quote's page only.

Deviations, each deliberate:
- **Early-stop suffix check fixed.** Upstream compares
  `strippedText.slice(match.end, strippedSuffix.length)` (a length used
  as an end offset), so its exact-suffix test is almost always false.
  This port slices `[end, end + len)`, which is what the comment beside
  it describes. It can only make the search stop earlier on a match that
  is already exact in quote and suffix.
- **A position hint of 0 counts.** Upstream tests `if (positionHint)`,
  so a quote at the very start of the document is searched without a
  hint. Here `Some(0)` is a hint like any other.
- **Page selector as a hint (kovan's addition).** Upstream's text
  anchoring ignores the page selector. Here, when there is no position
  selector, a page selector orders the pages by distance from it, so a
  `page` + `quote` anchor searches its own page first.
- No session cache of quote matches and no placeholder for unrendered
  pages: both belong to the viewer, not the anchoring.

```rust
pub mod pages { /* ... */ }
```

### Functions

#### Function `describe_in_pages`

Selectors for `pages[page][start..end]` (`char` offsets within that
page): the document-wide position, the quote with context from that
page, and the page (label defaults to `page + 1`). Upstream pdf.ts
`describe`. `None` if `page` is out of range.

```rust
pub fn describe_in_pages<P: AsRef<str>>(pages: &[P], page: usize, start: usize, end: usize, label: Option<String>) -> Option<Vec<super::selector::Selector>> { /* ... */ }
```

#### Function `anchor_in_pages`

Anchor `selectors` in a paged text (upstream pdf.ts `anchorRange` and
`anchorQuote`). A quote selector is required, as upstream: a position
alone cannot be checked against anything. The returned [`Anchor`] has
`page: Some(i)` and `char` offsets within that page's text.

```rust
pub fn anchor_in_pages<P: AsRef<str>>(pages: &[P], selectors: &[super::selector::Selector]) -> super::Anchoring { /* ... */ }
```

## Module `selector`

The selectors an annotation carries, in the W3C JSON shape
(`{"type": "TextQuoteSelector", "exact": …, "prefix": …, "suffix": …}`).

**Taken:** `TextQuoteSelector`, `TextPositionSelector` (W3C) and
`PageSelector` (Hypothesis's structural selector for paged documents:
0-based `index` plus an optional printed `label`).

**Left out**, as they describe a DOM or media and not plain text, lines
of code or pages of extracted text: `RangeSelector` (XPath into a DOM),
`EPUBContentSelector`, `MediaTimeSelector` and `ShapeSelector`. A
selector of a type this module does not know deserialises as
[`Selector::Unsupported`] and is ignored by the anchoring, as upstream
ignores types it has no anchor for; it cannot be serialised back.

**Offsets are Unicode scalar values (`char`s)**, as the W3C model
specifies ("the selection of the text MUST be in terms of unicode code
points"), not UTF-16 code units as Hypothesis stores them. The two agree
on any text without characters outside the Basic Multilingual Plane.

```rust
pub mod selector { /* ... */ }
```

### Types

#### Struct `TextQuoteSelector`

W3C `TextQuoteSelector`: the exact text, with some text before and after
it to tell repeated occurrences apart.

```rust
pub struct TextQuoteSelector {
    pub exact: String,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `exact` | `String` | The selected text, exactly. |
| `prefix` | `Option<String>` | Text immediately before `exact` (Hypothesis takes 32 characters). |
| `suffix` | `Option<String>` | Text immediately after `exact` (32 characters). |

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
    fn clone(self: &Self) -> TextQuoteSelector { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &TextQuoteSelector) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `TextPositionSelector`

W3C `TextPositionSelector`: `[start, end)` in `char`s from the start of
the text (for paged text, from the start of the first page).

```rust
pub struct TextPositionSelector {
    pub start: usize,
    pub end: usize,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `start` | `usize` | Offset of the first selected character. |
| `end` | `usize` | Offset one past the last selected character. |

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
    fn clone(self: &Self) -> TextPositionSelector { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &TextPositionSelector) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Struct `PageSelector`

Hypothesis's `PageSelector`: the page of a paged document.

```rust
pub struct PageSelector {
    pub index: usize,
    pub label: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `index` | `usize` | 0-based index of the page in the document's page sequence. |
| `label` | `Option<String>` | The printed page number, or the 1-based page number when the pages<br>carry none. |

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
    fn clone(self: &Self) -> PageSelector { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &PageSelector) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
#### Enum `Selector`

**Attributes:**

- `Other("#[serde(tag = \"type\")]")`

One selector, tagged by `type` as in the W3C JSON.

```rust
pub enum Selector {
    TextQuoteSelector(TextQuoteSelector),
    TextPositionSelector(TextPositionSelector),
    PageSelector(PageSelector),
    Unsupported,
}
```

##### Variants

###### `TextQuoteSelector`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `TextQuoteSelector` |  |

###### `TextPositionSelector`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `TextPositionSelector` |  |

###### `PageSelector`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `PageSelector` |  |

###### `Unsupported`

Any other `type` (module doc). Ignored when anchoring; serialising it
is an error, because what it held was not kept.

##### Implementations

###### Methods

- ```rust
  pub fn quote(selectors: &[Selector]) -> Option<&TextQuoteSelector> { /* ... */ }
  ```
  The first quote selector in `selectors`.

- ```rust
  pub fn position(selectors: &[Selector]) -> Option<&TextPositionSelector> { /* ... */ }
  ```
  The first position selector in `selectors`.

- ```rust
  pub fn page(selectors: &[Selector]) -> Option<&PageSelector> { /* ... */ }
  ```
  The first page selector in `selectors`.

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
    fn clone(self: &Self) -> Selector { /* ... */ }
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
- **Eq**
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
    fn eq(self: &Self, other: &Selector) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
## Module `text`

Describe and anchor a range of one plain text: a note, a source file, a
page. Upstream's HTML path with the DOM taken out: the "root element's
text content" is simply the text.

The line helpers at the end are kovan's own (not upstream): they convert
between `char` ranges and the 1-based inclusive line ranges review stamps
record, so a stamp's `lines` can be described and re-anchored.

```rust
pub mod text { /* ... */ }
```

### Functions

#### Function `describe_quote`

The quote selector for `text[start..end]` (`char` offsets, clamped to
the text): the exact text and up to [`CONTEXT_LEN`] characters either
side. Upstream `TextQuoteAnchor.fromRange`; prefix and suffix are always
present, empty at the ends of the text, as upstream.

```rust
pub fn describe_quote(text: &str, start: usize, end: usize) -> super::selector::TextQuoteSelector { /* ... */ }
```

#### Function `describe`

The selectors for `text[start..end]`: position, then quote. Upstream
`describe` (html.ts) without the DOM-only `RangeSelector` and
`MediaTimeSelector`.

```rust
pub fn describe(text: &str, start: usize, end: usize) -> Vec<super::selector::Selector> { /* ... */ }
```

#### Function `anchor`

Anchor `selectors` in `text`, upstream's order (html.ts `anchor`):

1. **Position**, accepted only if the text there equals the quote's
   `exact` (when there is a quote; with no quote it is accepted
   unverified, as upstream).
2. **Quote**, via [`super::match_quote()`]: exact occurrences first, and
   only if there are none the fuzzy search, both ranked by quote,
   prefix, suffix and nearness to the position selector's `start`.

Page selectors are ignored here (see [`super::anchor_in_pages`]).

```rust
pub fn anchor(text: &str, selectors: &[super::selector::Selector]) -> super::Anchoring { /* ... */ }
```

#### Function `lines_to_range`

The `char` range of 1-based inclusive lines `first..=last` of `text`,
from the first character of `first` to the end of `last` (its newline
excluded). `None` if the lines do not exist. Kovan's, not upstream.

```rust
pub fn lines_to_range(text: &str, first: usize, last: usize) -> Option<(usize, usize)> { /* ... */ }
```

#### Function `range_to_lines`

The 1-based inclusive lines that `text[start..end]` (`char` offsets)
touches. A range ending just after a newline does not count the next
line. Kovan's, not upstream.

```rust
pub fn range_to_lines(text: &str, start: usize, end: usize) -> (usize, usize) { /* ... */ }
```

### Constants and Statics

#### Constant `CONTEXT_LEN`

Characters of context captured on each side of a quote (upstream's
fixed `contextLen`).

```rust
pub const CONTEXT_LEN: usize = 32;
```

### Types

#### Enum `AnchorStrategy`

How an anchor was found.

```rust
pub enum AnchorStrategy {
    Position,
    PositionUnverified,
    ExactQuote,
    FuzzyQuote,
}
```

##### Variants

###### `Position`

The position selector, with the text there equal to the quote.

###### `PositionUnverified`

The position selector, with no quote to check it against.

###### `ExactQuote`

An exact occurrence of the quote (for paged text: exact once
whitespace is ignored).

###### `FuzzyQuote`

An approximate occurrence of the quote.

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
    fn clone(self: &Self) -> AnchorStrategy { /* ... */ }
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
    fn eq(self: &Self, other: &AnchorStrategy) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `Anchor`

Where an annotation anchored.

```rust
pub struct Anchor {
    pub page: Option<usize>,
    pub start: usize,
    pub end: usize,
    pub strategy: AnchorStrategy,
    pub score: f64,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `page` | `Option<usize>` | The page, for [`anchor_in_pages`]; `None` for [`anchor`]. |
| `start` | `usize` | Start (`char` offset; within the page for paged text). |
| `end` | `usize` | End, exclusive. |
| `strategy` | `AnchorStrategy` | Which step found it. |
| `score` | `f64` | Upstream's match score in [0, 1]; 1.0 for a verified position. |

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
    fn clone(self: &Self) -> Anchor { /* ... */ }
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
    fn eq(self: &Self, other: &Anchor) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Enum `OrphanReason`

Why an annotation could not be anchored.

```rust
pub enum OrphanReason {
    NoUsableSelector,
    NotFound,
}
```

##### Variants

###### `NoUsableSelector`

No selector this module can anchor (for paged text: no quote).

###### `NotFound`

The selectors were usable but nothing in the text matches them.

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
    fn clone(self: &Self) -> OrphanReason { /* ... */ }
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
    fn eq(self: &Self, other: &OrphanReason) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Enum `Anchoring`

The outcome of anchoring.

```rust
pub enum Anchoring {
    Anchored(Anchor),
    Orphaned(OrphanReason),
}
```

##### Variants

###### `Anchored`

Found, with how and how well.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `Anchor` |  |

###### `Orphaned`

Could not be anchored; the annotation is orphaned.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `OrphanReason` |  |

##### Implementations

###### Methods

- ```rust
  pub fn anchor(self: &Self) -> Option<&Anchor> { /* ... */ }
  ```
  The anchor, if any.

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
    fn clone(self: &Self) -> Anchoring { /* ... */ }
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
    fn eq(self: &Self, other: &Anchoring) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
### Re-exports

#### Re-export `match_quote`

```rust
pub use match_quote::match_quote;
```

#### Re-export `QuoteContext`

```rust
pub use match_quote::QuoteContext;
```

#### Re-export `QuoteMatch`

```rust
pub use match_quote::QuoteMatch;
```

#### Re-export `anchor_in_pages`

```rust
pub use pages::anchor_in_pages;
```

#### Re-export `describe_in_pages`

```rust
pub use pages::describe_in_pages;
```

#### Re-export `PageSelector`

```rust
pub use selector::PageSelector;
```

#### Re-export `Selector`

```rust
pub use selector::Selector;
```

#### Re-export `TextPositionSelector`

```rust
pub use selector::TextPositionSelector;
```

#### Re-export `TextQuoteSelector`

```rust
pub use selector::TextQuoteSelector;
```

#### Re-export `anchor`

```rust
pub use text::anchor;
```

#### Re-export `describe`

```rust
pub use text::describe;
```

#### Re-export `describe_quote`

```rust
pub use text::describe_quote;
```

#### Re-export `lines_to_range`

```rust
pub use text::lines_to_range;
```

#### Re-export `range_to_lines`

```rust
pub use text::range_to_lines;
```

## Module `zotero`

Zotero's data model, ported so Zotero libraries can be imported into and
exported from kovan (GitHub #748, epic #747).

| Module | What | Ported from |
|---|---|---|
| [`schema_generated`] | `ItemType`, `Field`, `CreatorType` enums; every item type's fields, base fields and creator types; `meta`; the CSL mappings; en-US labels | zotero-schema `schema.json` v45, generated by `kovan_codegen::zotero` |
| [`schema`] | field validity, base-field <-> type-specific field mapping, primary creator types | `itemFields.js`, `cachedTypes.js` |
| [`item`] | [`ZoteroItem`], [`ZoteroCollection`], [`ZoteroLibrary`]: the Web API / translator JSON shape | `item.js` `toJSON`/`fromJSON`, `collection.js` |
| [`validate`] | schema checks and `fromJSON`'s move of invalid fields into Extra | `item.js`, `utilities_internal.js` |
| [`date`] | `strToDate`, `parseEDTF`, `formatDate`, SQL/ISO dates | utilities `date.js` |
| [`csl`] | CSL-JSON both ways | utilities `utilities_item.js` |
| [`search`] | [`ZoteroSearch`]: a saved search, conditions as stored (added 2026-10-07, #750) | `search.js` `toJSON`/`fromJSON` |
| [`kovan`] | [`ZoteroItem`] <-> [`crate::KovanDocument`], with the lossy fields listed | (kovan's own) |

Every value the ports were checked against comes from upstream's own data
(schema.json and Zotero's test suites); upstream code was read, never run.
Running upstream for code-to-code verification is GitHub #752.

**Maturity: AI draft (1).** Not yet human-reviewed.

```rust
pub mod zotero { /* ... */ }
```

### Modules

## Module `csl`

CSL-JSON <-> Zotero item, ported from `Zotero.Utilities.Item`.

[`item_to_csl_json`] is `itemToCSLJSON` applied to the export format of an
item (the JSON [`ZoteroItem`] holds); [`item_from_csl_json`] is
`itemFromCSLJSON` filling a fresh `Zotero.Item` (the `isZoteroItem` path,
so base-mapped values land in the type-specific field, as `setField`
stores them). A CSL item is a JSON object, [`CslItem`].

Differences from upstream, all deliberate:

* dates go through [`super::date`] (en-US months, configurable "local"
  time zone; see that module);
* `noteToTitle` strips tags and decodes entities with a small decoder
  instead of a DOM (`textContent`); block-level whitespace can differ for
  unusual HTML;
* the CSL `id` is the item's `uri` (as for an export-format item) and is
  omitted when there is none; `itemFromCSLJSON` ignores the CSL `id`, as
  the `isZoteroItem` path does.

```rust
pub mod csl { /* ... */ }
```

### Types

#### Type Alias `CslItem`

A CSL-JSON item.

```rust
pub type CslItem = serde_json::Map<String, serde_json::Value>;
```

#### Enum `CslError`

Why a conversion failed.

```rust
pub enum CslError {
    UnexpectedItemType(super::schema_generated::ItemType),
    MissingType,
}
```

##### Variants

###### `UnexpectedItemType`

`itemToCSLJSON`: the item type has no CSL type (`annotation`);
upstream throws `Unexpected Zotero Item type "..."` (:176).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::schema_generated::ItemType` |  |

###### `MissingType`

`itemFromCSLJSON`: no `type` (:424-427).

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
    fn clone(self: &Self) -> CslError { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CslError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `csl_date_to_edtf`

`cslDateToEDTF` (:68): a CSL date with a range or circa flag as EDTF, or
`None`.

```rust
pub fn csl_date_to_edtf(d: &serde_json::Map<String, serde_json::Value>) -> Option<String> { /* ... */ }
```

#### Function `date_exports_as_literal`

`dateExportsAsLiteral` (:134).

```rust
pub fn date_exports_as_literal(s: &str, opts: &super::date::DateOptions) -> bool { /* ... */ }
```

#### Function `unescape_html`

`Zotero.Utilities.unescapeHTML` (utilities.js:699) without a DOM: strip
tags, decode the common named and all numeric entities, collapse runs of
spaces.

```rust
pub fn unescape_html(s: &str) -> String { /* ... */ }
```

#### Function `note_to_title`

`noteToTitle` (:806): the first line (at most 120 characters) of a
note's HTML as plain text.

```rust
pub fn note_to_title(text: &str, stop_at_line_break: bool) -> String { /* ... */ }
```

#### Function `extra_to_csl`

`extraToCSL` (:844): rename `Field: value` lines of Extra to CSL
variable names.

```rust
pub fn extra_to_csl(extra: &str) -> String { /* ... */ }
```

#### Function `parse_particles`

`parseParticles` (:683): split name particles out of a CSL name object
with `family` and `given` set ("Jean de" + "la Fontaine" -> given "Jean",
dropping "de", non-dropping "la", family "Fontaine").

```rust
pub fn parse_particles(name: &mut serde_json::Map<String, serde_json::Value>) { /* ... */ }
```

#### Function `item_to_csl_json`

`itemToCSLJSON` (:161) with [`DateOptions::default`].

```rust
pub fn item_to_csl_json(item: &super::item::ZoteroItem) -> Result<CslItem, CslError> { /* ... */ }
```

#### Function `item_to_csl_json_with`

`itemToCSLJSON` (:161): a Zotero item (export format) as a CSL item.

# Errors
[`CslError::UnexpectedItemType`] for an `annotation`.

```rust
pub fn item_to_csl_json_with(item: &super::item::ZoteroItem, opts: &super::date::DateOptions) -> Result<CslItem, CslError> { /* ... */ }
```

#### Function `item_from_csl_json`

`itemFromCSLJSON` (:417) with [`DateOptions::default`].

```rust
pub fn item_from_csl_json(csl: &CslItem) -> Result<super::item::ZoteroItem, CslError> { /* ... */ }
```

#### Function `item_type_for_csl`

The item type `itemFromCSLJSON` picks for a CSL item (:432-468).

```rust
pub fn item_type_for_csl(csl: &CslItem) -> Option<super::schema_generated::ItemType> { /* ... */ }
```

#### Function `item_from_csl_json_with`

`itemFromCSLJSON` (:417): a CSL item as a new Zotero item.

# Errors
[`CslError::MissingType`] when the CSL item has no `type`.

```rust
pub fn item_from_csl_json_with(csl_in: &CslItem, opts: &super::date::DateOptions) -> Result<super::item::ZoteroItem, CslError> { /* ... */ }
```

## Module `date`

Zotero's date handling (`Zotero.Date`), the parts that the CSL-JSON
conversion and the Zotero item model need.

Ported: `strToDate` (date.js:272), `formatDate` (long form, :567),
`strToISO` (:600), `sqlToISO8601` (:617), `parseEraYear` (:744),
`looksLikeEDTF` (:766), `parseEDTF` (:799), `strToMultipart` (:902),
`multipartToSQL` (:966), `isMultipart` (:954) and `multipartToStr` (:983)
(added 2026-10-07, #750), `isSQLDate`/`isSQLDateTime`/
`isSQLDateTimeWithoutSeconds` (:1019-1034), `isISODate` (:235), and the
ISO -> SQL and UTC -> local conversions of `isoToDate`/`dateToSQL`/
`sqlToDate`.

**Deliberate differences, all stated:**

* **Locale.** Month names are en-US only (Zotero adds the UI locale's
  months to English ones; date.js:83-97). A Chinese or French month name is
  not recognised. Two-digit-year windowing and US/European day-month order
  come from [`DateOptions`] instead of the clock and `Zotero.locale`.
* **Time zone.** JavaScript's `Date` uses the machine's zone; here "local
  time" is UTC shifted by [`DateOptions::utc_offset_minutes`] (default 0,
  i.e. local = UTC). No daylight-saving rules.
* **`undefined` parts.** Upstream stores a non-participating regex group
  as `undefined` and later concatenates it into `part`, which yields the
  literal text `"undefined"` (e.g. `strToDate("2021 May").part`). This port
  reproduces that, because upstream is the specification; it is marked
  where it happens.

```rust
pub mod date { /* ... */ }
```

### Types

#### Struct `DateOptions`

What upstream reads from the environment: the clock, the locale and the
time zone.

```rust
pub struct DateOptions {
    pub current_year: i64,
    pub month_first: bool,
    pub day_suffixes: Vec<String>,
    pub utc_offset_minutes: i32,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `current_year` | `i64` | The current year, for two-digit years (date.js:351-364: a two-digit<br>year up to this year's last two digits is this century, otherwise the<br>previous one). |
| `month_first` | `bool` | Read an ambiguous `a/b/c` as month/day (US, FM, PW, PH locales,<br>date.js:310-314) rather than day/month. |
| `day_suffixes` | `Vec<String>` | Day suffixes accepted after a day number ("26th"). The Zotero client<br>uses the locale string `date.daySuffixes` ("st, nd, rd, th" for<br>en-US); the standalone utilities (no client) use none (date.js:434). |
| `utc_offset_minutes` | `i32` | Minutes east of UTC of "local time" (`Date#getTimezoneOffset` with<br>the sign flipped). Used only for the access-date conversion in<br>`itemToCSLJSON` (utilities_item.js:306-315). |

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
    fn clone(self: &Self) -> DateOptions { /* ... */ }
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
    The Zotero client in en-US, UTC, with `current_year` from the system

- **Eq**
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
    fn eq(self: &Self, other: &DateOptions) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Type Alias `YearString`

The year of a [`StrDate`], which upstream keeps as a string ("2010",
"200 BCE", "circa 1995").

```rust
pub type YearString = String;
```

#### Struct `StrDate`

The result of [`str_to_date`] (upstream's plain object).

```rust
pub struct StrDate {
    pub year: Option<YearString>,
    pub month: Option<u32>,
    pub day: Option<u32>,
    pub part: Option<String>,
    pub order: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `year` | `Option<YearString>` | The year as a string, possibly with an era marker or circa prefix. |
| `month` | `Option<u32>` | The month, **0-indexed** as in JavaScript (January = 0). |
| `day` | `Option<u32>` | The day of the month (1-31). |
| `part` | `Option<String>` | Anything left over (e.g. a season "Summer"), cleaned of punctuation<br>at both ends. |
| `order` | `String` | The order the parts appeared in: a string over `y`, `m`, `d`. |

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
    fn clone(self: &Self) -> StrDate { /* ... */ }
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
    fn default() -> StrDate { /* ... */ }
    ```

- **Eq**
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
    fn eq(self: &Self, other: &StrDate) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `EdtfParts`

Year, month (0-indexed) and day of one end of a [`EdtfDate`].

```rust
pub struct EdtfParts {
    pub year: i64,
    pub month: Option<u32>,
    pub day: Option<u32>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `year` | `i64` | Signed year (1 BCE = 0, as in EDTF). |
| `month` | `Option<u32>` | Month, 0-indexed. |
| `day` | `Option<u32>` | Day of the month. |

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
    fn clone(self: &Self) -> EdtfParts { /* ... */ }
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
    fn eq(self: &Self, other: &EdtfParts) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `EdtfDate`

The result of [`parse_edtf`].

```rust
pub struct EdtfDate {
    pub begin: EdtfParts,
    pub end: Option<EdtfParts>,
    pub circa: bool,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `begin` | `EdtfParts` | The date, or the start of the interval. |
| `end` | `Option<EdtfParts>` | The end of a closed interval. |
| `circa` | `bool` | Whether any part is uncertain or approximate (`~`, `?`, `%`). |

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
    fn clone(self: &Self) -> EdtfDate { /* ... */ }
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
    fn eq(self: &Self, other: &EdtfDate) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `parse_era_year`

`Zotero.Date.parseEraYear` (date.js:744): a single year with an era
marker ("200 BCE", "AD 200") as a signed year; `None` otherwise.

```rust
pub fn parse_era_year(s: &str) -> Option<i64> { /* ... */ }
```

#### Function `looks_like_edtf`

`Zotero.Date.looksLikeEDTF` (date.js:766): an EDTF-shaped value, a
negative year or an era marker.

```rust
pub fn looks_like_edtf(s: &str) -> bool { /* ... */ }
```

#### Function `parse_edtf`

`Zotero.Date.parseEDTF` (date.js:799): the subset of EDTF (ISO 8601-2)
that Zotero reads, plus its normalisations (dash ranges, condensed ranges,
circa prefixes, era markers). `None` where upstream returns `false`.

```rust
pub fn parse_edtf(s: &str) -> Option<EdtfDate> { /* ... */ }
```

#### Function `lpad`

`Zotero.Utilities.lpad` (utilities.js:975).

```rust
pub fn lpad(s: &str, pad: char, length: usize) -> String { /* ... */ }
```

#### Function `trim_internal`

`Zotero.Utilities.trimInternal` (utilities.js:310): collapse whitespace
runs to one space and trim.

```rust
pub fn trim_internal(s: &str) -> String { /* ... */ }
```

#### Function `str_to_date`

`Zotero.Date.strToDate` (date.js:272): read year, month, day and a
leftover part out of free text. See the module docs for the locale and
`undefined` notes.

```rust
pub fn str_to_date(s: &str, opts: &DateOptions) -> StrDate { /* ... */ }
```

#### Function `format_date`

`Zotero.Date.formatDate` without `shortFormat` (date.js:567): e.g.
"Summer December 31, 1999". The month is 0-indexed.

```rust
pub fn format_date(part: Option<&str>, year: Option<&str>, month: Option<u32>, day: Option<u32>) -> String { /* ... */ }
```

#### Function `str_to_iso`

`Zotero.Date.strToISO` (date.js:600): `YYYY[-MM[-DD]]`, or `None`.

```rust
pub fn str_to_iso(s: &str, opts: &DateOptions) -> Option<String> { /* ... */ }
```

#### Function `is_sql_date`

`Zotero.Date.isSQLDate` (date.js:1019), `YYYY-MM-DD`.

```rust
pub fn is_sql_date(s: &str) -> bool { /* ... */ }
```

#### Function `is_sql_date_time`

`Zotero.Date.isSQLDateTime` (date.js:1027), `YYYY-MM-DD hh:mm:ss`.

```rust
pub fn is_sql_date_time(s: &str) -> bool { /* ... */ }
```

#### Function `is_sql_date_time_without_seconds`

`Zotero.Date.isSQLDateTimeWithoutSeconds` (date.js:1032).

```rust
pub fn is_sql_date_time_without_seconds(s: &str) -> bool { /* ... */ }
```

#### Function `is_iso_date`

`Zotero.Date.isISODate` (date.js:235).

```rust
pub fn is_iso_date(s: &str) -> bool { /* ... */ }
```

#### Function `sql_to_epoch_secs`

`Zotero.Date.sqlToDate(sql, isUTC=true)` as seconds since the epoch, for
`YYYY-MM-DD`, `YYYY-MM-DD hh:mm` and `YYYY-MM-DD hh:mm:ss` (date.js:121).

```rust
pub fn sql_to_epoch_secs(sql: &str) -> Option<i64> { /* ... */ }
```

#### Function `iso_to_sql`

`Zotero.Date.isoToDate` then `dateToSQL(d, true)` (date.js:245, 167):
an ISO 8601 date/time as a UTC SQL date-time. A date-only or
zone-less value is read as UTC (JavaScript reads a zone-less *date-time*
as local time; with local = UTC, as here, the two agree).

```rust
pub fn iso_to_sql(iso: &str) -> Option<String> { /* ... */ }
```

#### Function `utc_sql_to_local_sql`

UTC SQL date-time -> "local" SQL date-time, shifted by
[`DateOptions::utc_offset_minutes`] (`sqlToDate(date, true)` then
`dateToSQL(localDate)`, utilities_item.js:313-314). Invalid input gives
`""`, as upstream's `dateToSQL(false)` does.

```rust
pub fn utc_sql_to_local_sql(sql: &str, opts: &DateOptions) -> String { /* ... */ }
```

#### Function `sql_to_iso8601`

`Zotero.Date.sqlToISO8601` (date.js:617): drop `-00` parts, `T...Z`.

```rust
pub fn sql_to_iso8601(sql: &str) -> Option<String> { /* ... */ }
```

#### Function `str_to_multipart`

`Zotero.Date.strToMultipart` (date.js:902): the sortable
`YYYY-MM-DD <original>` form Zotero stores for date fields.

```rust
pub fn str_to_multipart(s: &str, opts: &DateOptions) -> String { /* ... */ }
```

#### Function `multipart_to_sql`

`Zotero.Date.multipartToSQL` (date.js:966): the `YYYY-MM-DD` head of a
multipart date, `0000-00-00` for a non-multipart string.

```rust
pub fn multipart_to_sql(multi: &str) -> String { /* ... */ }
```

#### Function `is_multipart`

`Zotero.Date.isMultipart` (date.js:954): whether `s` is a multipart date
(`YYYY-MM-DD <original>`), which an SQL date-time is not. Added
2026-10-07 for the Zotero database reader (GitHub #750).

```rust
pub fn is_multipart(s: &str) -> bool { /* ... */ }
```

#### Function `multipart_to_str`

`Zotero.Date.multipartToStr` (date.js:983): the user part of a multipart
date (`2006-11-03 November 3rd, 2006` -> `November 3rd, 2006`); any other
string unchanged. This is how `Item#getField` turns a stored date field
back into what the user typed (item.js:303). Added 2026-10-07 (#750).

```rust
pub fn multipart_to_str(multi: &str) -> String { /* ... */ }
```

## Module `item`

The Zotero item, collection and library model, in Zotero's Web API /
translator JSON shape.

[`ZoteroItem`] (de)serialises the JSON that `Zotero.Item#toJSON` writes and
`Zotero.Item#fromJSON` reads (also the Web API's `data` object, and,
with `uri`, `attachments` and `notes`, the translator export format of
`itemToExportFormat`). Reading is lenient where upstream is lenient:

* a number in a field becomes a string (item.js:779);
* a string under a name that is not a Zotero field, or a field that is
  not valid for the item type, is **kept** in [`ZoteroItem::fields`]
  (upstream moves it into Extra on import; call
  [`ZoteroItem::normalize_fields`] to do that, and
  [`ZoteroItem::validate`] to list such problems);
* any other unknown property is kept verbatim in
  [`ZoteroItem::other`], so it survives a round trip.

Reading is strict where upstream is strict: an unknown `itemType`
(item.js:5664), an unknown `linkMode` (:5784) or an unknown creator type is
an error.

```rust
pub mod item { /* ... */ }
```

### Types

#### Enum `LinkMode`

How an attachment's file is held (`Zotero.Attachments.LINK_MODE_*`,
attachments.js:35-39; JSON names from `linkModeToName`, :3436).

```rust
pub enum LinkMode {
    ImportedFile,
    ImportedUrl,
    LinkedFile,
    LinkedUrl,
    EmbeddedImage,
}
```

##### Variants

###### `ImportedFile`

`imported_file`: a file stored in Zotero's storage.

###### `ImportedUrl`

`imported_url`: a web snapshot stored in Zotero's storage.

###### `LinkedFile`

`linked_file`: a file elsewhere on disk (`path` is set).

###### `LinkedUrl`

`linked_url`: a link to a web page, no file.

###### `EmbeddedImage`

`embedded_image`: an image embedded in a note.

##### Implementations

###### Methods

- ```rust
  pub const fn as_str(self: Self) -> &'static str { /* ... */ }
  ```
  The JSON name.

- ```rust
  pub fn from_name(name: &str) -> Option<LinkMode> { /* ... */ }
  ```
  Parse a JSON name. Upstream looks up `LINK_MODE_` + the upper-cased

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
    fn clone(self: &Self) -> LinkMode { /* ... */ }
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
    fn eq(self: &Self, other: &LinkMode) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Enum `AnnotationType`

The kind of a Zotero 7 annotation (`Zotero.Annotations.ANNOTATION_TYPE_*`,
annotations.js:31-36).

```rust
pub enum AnnotationType {
    Highlight,
    Note,
    Image,
    Ink,
    Underline,
    Text,
}
```

##### Variants

###### `Highlight`

`highlight`

###### `Note`

`note`

###### `Image`

`image`

###### `Ink`

`ink`

###### `Underline`

`underline`

###### `Text`

`text`

##### Implementations

###### Methods

- ```rust
  pub const fn as_str(self: Self) -> &'static str { /* ... */ }
  ```
  The JSON name.

- ```rust
  pub fn from_name(name: &str) -> Option<AnnotationType> { /* ... */ }
  ```
  Parse a JSON name (exact match).

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
    fn clone(self: &Self) -> AnnotationType { /* ... */ }
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
    fn eq(self: &Self, other: &AnnotationType) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Enum `CreatorName`

A creator's name: two fields, or one (`fieldMode: 1`, e.g. an institution).

```rust
pub enum CreatorName {
    TwoField {
        first_name: String,
        last_name: String,
    },
    SingleField {
        name: String,
    },
}
```

##### Variants

###### `TwoField`

`firstName` + `lastName`.

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `first_name` | `String` | Given name(s). |
| `last_name` | `String` | Family name. |

###### `SingleField`

`name` (written by `toJSON` for `fieldMode: 1`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `name` | `String` | The whole name. |

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
    fn clone(self: &Self) -> CreatorName { /* ... */ }
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
    fn eq(self: &Self, other: &CreatorName) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `Creator`

One creator of an item.

```rust
pub struct Creator {
    pub creator_type: super::schema_generated::CreatorType,
    pub name: CreatorName,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `creator_type` | `super::schema_generated::CreatorType` | The role, e.g. `author`, `editor`. |
| `name` | `CreatorName` | The name. |

##### Implementations

###### Methods

- ```rust
  pub fn person</* synthetic */ impl Into<String>: Into<String>, /* synthetic */ impl Into<String>: Into<String>>(creator_type: CreatorType, first_name: impl Into<String>, last_name: impl Into<String>) -> Self { /* ... */ }
  ```
  A two-field creator.

- ```rust
  pub fn single</* synthetic */ impl Into<String>: Into<String>>(creator_type: CreatorType, name: impl Into<String>) -> Self { /* ... */ }
  ```
  A single-field creator (an organisation).

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
    fn clone(self: &Self) -> Creator { /* ... */ }
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
    fn eq(self: &Self, other: &Creator) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `Tag`

One tag. `tag_type` is `None`/`Some(0)` for a manual tag and `Some(1)`
for an automatic one (imported with the metadata).

```rust
pub struct Tag {
    pub tag: String,
    pub tag_type: Option<u8>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tag` | `String` | The tag text. |
| `tag_type` | `Option<u8>` | The JSON `type`; absent means 0 (manual). |

##### Implementations

###### Methods

- ```rust
  pub fn is_automatic(self: &Self) -> bool { /* ... */ }
  ```
  Whether this is an automatic tag (`type: 1`).

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
    fn clone(self: &Self) -> Tag { /* ... */ }
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
    fn eq(self: &Self, other: &Tag) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `AttachmentData`

The attachment-only properties of an item (item.js:6038-6079, 5783-5806).

```rust
pub struct AttachmentData {
    pub link_mode: Option<LinkMode>,
    pub content_type: Option<String>,
    pub charset: Option<String>,
    pub path: Option<String>,
    pub filename: Option<String>,
    pub md5: Option<String>,
    pub mtime: Option<i64>,
    pub last_read: Option<i64>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `link_mode` | `Option<LinkMode>` | `linkMode`; `None` only for an attachment JSON that omitted it. |
| `content_type` | `Option<String>` | `contentType`, e.g. `application/pdf`. |
| `charset` | `Option<String>` | `charset`. |
| `path` | `Option<String>` | `path`, for `linked_file` (absolute, or `attachments:` relative to the<br>linked-attachment base directory). |
| `filename` | `Option<String>` | `filename`, for stored files. |
| `md5` | `Option<String>` | `md5` of the stored file (sync only). |
| `mtime` | `Option<i64>` | `mtime` of the stored file in ms (sync only). |
| `last_read` | `Option<i64>` | `lastRead` (Unix seconds; user library only). |

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
    fn clone(self: &Self) -> AttachmentData { /* ... */ }
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
    fn default() -> AttachmentData { /* ... */ }
    ```

- **Eq**
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
    fn eq(self: &Self, other: &AttachmentData) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `AnnotationData`

The annotation-only properties of an item (item.js:6089-6101, 5811-5820).

```rust
pub struct AnnotationData {
    pub annotation_type: Option<AnnotationType>,
    pub author_name: Option<String>,
    pub text: Option<String>,
    pub comment: Option<String>,
    pub color: Option<String>,
    pub page_label: Option<String>,
    pub sort_index: Option<String>,
    pub position: Option<String>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `annotation_type` | `Option<AnnotationType>` | `annotationType`; `None` only for an annotation JSON that omitted it. |
| `author_name` | `Option<String>` | `annotationAuthorName`. |
| `text` | `Option<String>` | `annotationText` (only highlights and underlines carry text). |
| `comment` | `Option<String>` | `annotationComment` (rich text). |
| `color` | `Option<String>` | `annotationColor`, e.g. `#ffd400`. |
| `page_label` | `Option<String>` | `annotationPageLabel`. |
| `sort_index` | `Option<String>` | `annotationSortIndex`, e.g. `00015|002431|00000`. |
| `position` | `Option<String>` | `annotationPosition`: a JSON **string** (e.g.<br>`{"pageIndex":1,"rects":[[...]]}`), kept as text as upstream does. |

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
    fn clone(self: &Self) -> AnnotationData { /* ... */ }
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
    fn default() -> AnnotationData { /* ... */ }
    ```

- **Eq**
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
    fn eq(self: &Self, other: &AnnotationData) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `ZoteroItem`

A Zotero item in Web API / translator JSON form.

```rust
pub struct ZoteroItem {
    pub key: Option<String>,
    pub version: Option<u64>,
    pub item_type: super::schema_generated::ItemType,
    pub fields: std::collections::BTreeMap<String, String>,
    pub creators: Vec<Creator>,
    pub tags: Vec<Tag>,
    pub collections: Vec<String>,
    pub relations: std::collections::BTreeMap<String, Vec<String>>,
    pub parent_item: Option<String>,
    pub note: Option<String>,
    pub attachment: Option<AttachmentData>,
    pub annotation: Option<AnnotationData>,
    pub deleted: Option<bool>,
    pub in_publications: Option<bool>,
    pub date_added: Option<String>,
    pub date_modified: Option<String>,
    pub uri: Option<String>,
    pub attachments: Vec<ZoteroItem>,
    pub notes: Vec<ZoteroItem>,
    pub other: std::collections::BTreeMap<String, serde_json::Value>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `key` | `Option<String>` | The 8-character object key. |
| `version` | `Option<u64>` | The object version (sync). |
| `item_type` | `super::schema_generated::ItemType` | The item type. |
| `fields` | `std::collections::BTreeMap<String, String>` | Field name -> value, as stored (type-specific names, e.g.<br>`websiteTitle` on a `webpage`). Includes `extra` and `accessDate`. |
| `creators` | `Vec<Creator>` | Creators, in order (regular items only). |
| `tags` | `Vec<Tag>` | Tags. |
| `collections` | `Vec<String>` | Keys of the collections a top-level item is in. |
| `relations` | `std::collections::BTreeMap<String, Vec<String>>` | Relations: predicate (e.g. `dc:relation`, `owl:sameAs`) -> object URIs. |
| `parent_item` | `Option<String>` | `parentItem`: the parent's key, for child notes, attachments and<br>annotations. |
| `note` | `Option<String>` | `note`: HTML of a note, or an attachment's note. |
| `attachment` | `Option<AttachmentData>` | Attachment properties; set for `attachment` items. |
| `annotation` | `Option<AnnotationData>` | Annotation properties; set for `annotation` items. |
| `deleted` | `Option<bool>` | `deleted` (in the trash). |
| `in_publications` | `Option<bool>` | `inPublications` (My Publications). |
| `date_added` | `Option<String>` | `dateAdded`, ISO 8601 UTC (e.g. `2015-04-12T09:00:22Z`). |
| `date_modified` | `Option<String>` | `dateModified`, ISO 8601 UTC. |
| `uri` | `Option<String>` | `uri` (translator export format; e.g.<br>`http://zotero.org/users/local/abc/items/KEY`); becomes the CSL `id`. |
| `attachments` | `Vec<ZoteroItem>` | Child attachments (translator export format). |
| `notes` | `Vec<ZoteroItem>` | Child notes (translator export format). |
| `other` | `std::collections::BTreeMap<String, serde_json::Value>` | Every other property, verbatim (e.g. `itemID`, `libraryID`, or<br>properties of a newer Zotero). |

##### Implementations

###### Methods

- ```rust
  pub fn new(item_type: ItemType) -> Self { /* ... */ }
  ```
  An empty item of the given type, with the attachment/annotation

- ```rust
  pub fn field(self: &Self, field: Field) -> Option<&str> { /* ... */ }
  ```
  The value stored under `field` (exact name, no base-field mapping).

- ```rust
  pub fn set_field</* synthetic */ impl Into<String>: Into<String>>(self: &mut Self, field: Field, value: impl Into<String>) { /* ... */ }
  ```
  Store `value` under `field` (exact name; an empty value removes it,

- ```rust
  pub fn field_via_base(self: &Self, base: Field) -> Option<&str> { /* ... */ }
  ```
  The value of a field read through its **base** field, as

- ```rust
  pub fn from_json_value(v: &Value) -> Result<Self, ZoteroJsonError> { /* ... */ }
  ```
  Parse Zotero item JSON (see the module docs for what is lenient and

- ```rust
  pub fn to_json_value(self: &Self) -> Value { /* ... */ }
  ```
  Write Zotero item JSON, in the shape of `Zotero.Item#toJSON`

- ```rust
  pub fn to_kovan_document(self: &Self) -> KovanDocument { /* ... */ }
  ```
  This item as a [`KovanDocument`] (see the module docs for the field

- ```rust
  pub fn from_kovan_document(doc: &KovanDocument) -> ZoteroItem { /* ... */ }
  ```
  A Zotero item for a [`KovanDocument`]: its stored `zotero_item` with

- ```rust
  pub fn validate(self: &Self) -> Vec<ValidationIssue> { /* ... */ }
  ```
  Every way this item departs from the schema (empty when it matches).

- ```rust
  pub fn normalize_fields(self: &mut Self) { /* ... */ }
  ```
  Repair the fields the way non-strict `Zotero.Item#fromJSON` does on

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
    fn clone(self: &Self) -> ZoteroItem { /* ... */ }
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
    fn deserialize<D: serde::Deserializer<''de>>(d: D) -> Result<Self, <D as >::Error> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
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
    fn eq(self: &Self, other: &ZoteroItem) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<S: serde::Serializer>(self: &Self, s: S) -> Result<<S as >::Ok, <S as >::Error> { /* ... */ }
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
#### Enum `ZoteroJsonError`

Why a JSON value is not a Zotero item or collection.

```rust
pub enum ZoteroJsonError {
    NotAnObject,
    MissingItemType,
    UnknownItemType(String),
    UnknownLinkMode(String),
    UnknownAnnotationType(String),
    UnknownCreatorType(String),
    BadProperty(String),
    MissingCollectionName,
}
```

##### Variants

###### `NotAnObject`

The value is not a JSON object.

###### `MissingItemType`

`itemType` is missing (item.js:5660).

###### `UnknownItemType`

`itemType` names no known item type (item.js:5664).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `UnknownLinkMode`

`linkMode` names no known link mode (item.js:5784).

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `UnknownAnnotationType`

`annotationType` names no known annotation type.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `UnknownCreatorType`

A creator's `creatorType` names no known creator type.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `BadProperty`

A property has the wrong JSON type; the payload names it.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `MissingCollectionName`

A collection has no `name` (collection.js:812).

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
    fn clone(self: &Self) -> ZoteroJsonError { /* ... */ }
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

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ZoteroJsonError) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `ZoteroCollection`

A Zotero collection (collection.js:826 `toJSON`, :792 `fromJSON`).

```rust
pub struct ZoteroCollection {
    pub key: Option<String>,
    pub version: Option<u64>,
    pub name: String,
    pub parent_collection: Option<String>,
    pub relations: std::collections::BTreeMap<String, Vec<String>>,
    pub deleted: Option<bool>,
    pub other: std::collections::BTreeMap<String, serde_json::Value>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `key` | `Option<String>` | The 8-character object key. |
| `version` | `Option<u64>` | The object version. |
| `name` | `String` | The collection name (required). |
| `parent_collection` | `Option<String>` | The parent collection's key; `None` for a top-level collection<br>(written as `false`, as upstream does). |
| `relations` | `std::collections::BTreeMap<String, Vec<String>>` | Relations, as for items. |
| `deleted` | `Option<bool>` | `deleted` (in the trash). |
| `other` | `std::collections::BTreeMap<String, serde_json::Value>` | Every other property, verbatim. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>) -> Self { /* ... */ }
  ```
  A top-level collection with the given name.

- ```rust
  pub fn from_json_value(v: &Value) -> Result<Self, ZoteroJsonError> { /* ... */ }
  ```
  Parse collection JSON.

- ```rust
  pub fn to_json_value(self: &Self) -> Value { /* ... */ }
  ```
  Write collection JSON as `toJSON` does.

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
    fn clone(self: &Self) -> ZoteroCollection { /* ... */ }
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
    fn deserialize<D: serde::Deserializer<''de>>(d: D) -> Result<Self, <D as >::Error> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
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
    fn eq(self: &Self, other: &ZoteroCollection) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<S: serde::Serializer>(self: &Self, s: S) -> Result<<S as >::Ok, <S as >::Error> { /* ... */ }
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
#### Struct `ZoteroLibrary`

A Zotero library: its collections, its items (top-level and child items
side by side, children pointing at parents through `parentItem`, as the
Web API lists them) and its saved searches.

This is a container for import/export, not a port of `Zotero.Library`
(which is a database handle). Serialises as
`{"collections": [...], "items": [...]}`, plus `"searches": [...]` when
there are saved searches.

```rust
pub struct ZoteroLibrary {
    pub collections: Vec<ZoteroCollection>,
    pub items: Vec<ZoteroItem>,
    pub searches: Vec<super::search::ZoteroSearch>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `collections` | `Vec<ZoteroCollection>` | Collections. |
| `items` | `Vec<ZoteroItem>` | Items, including child notes, attachments and annotations. |
| `searches` | `Vec<super::search::ZoteroSearch>` | Saved searches (search.js `toJSON`). Added 2026-10-07 (#750) for the<br>Zotero database reader; omitted from the JSON when empty, so a library<br>without searches serialises exactly as before. |

##### Implementations

###### Methods

- ```rust
  pub fn item(self: &Self, key: &str) -> Option<&ZoteroItem> { /* ... */ }
  ```
  The item with this key.

- ```rust
  pub fn children(self: &Self, parent_key: &str) -> impl Iterator<Item = &ZoteroItem> { /* ... */ }
  ```
  The child items (notes, attachments, annotations) of `parent_key`.

- ```rust
  pub fn items_in_collection(self: &Self, collection_key: &str) -> impl Iterator<Item = &ZoteroItem> { /* ... */ }
  ```
  The items in the collection with key `collection_key`.

- ```rust
  pub fn subcollections(self: &Self, parent_key: Option<&str>) -> impl Iterator<Item = &ZoteroCollection> { /* ... */ }
  ```
  The child collections of `parent_key` (`None` for top-level ones).

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
    fn clone(self: &Self) -> ZoteroLibrary { /* ... */ }
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
    fn default() -> ZoteroLibrary { /* ... */ }
    ```

- **Deserialize**
  - ```rust
    fn deserialize<__D>(__deserializer: __D) -> _serde::__private228::Result<Self, <__D as >::Error>
where
    __D: _serde::Deserializer<''de> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
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
    fn eq(self: &Self, other: &ZoteroLibrary) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<__S>(self: &Self, __serializer: __S) -> _serde::__private228::Result<<__S as >::Ok, <__S as >::Error>
where
    __S: _serde::Serializer { /* ... */ }
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
## Module `kovan`

[`ZoteroItem`] <-> [`KovanDocument`].

## Zotero -> kovan ([`ZoteroItem::to_kovan_document`])

| `KovanDocument` | from the Zotero item |
|---|---|
| `id` | `"zotero:" + key` (`"zotero:"` when the item has no key) |
| `slug` | `citationKey`, else `<first author family><year>`, lower-case ASCII |
| `visibility` | always `Proprietary` (Zotero records no redistribution right; `DATA_POLICY.md` makes "may not be committed" the safe default) |
| `document_type` | journal/magazine/newspaper article, conference paper, preprint -> `Paper`; `report` -> `Report`; `standard` -> `Standard`; `thesis` -> `Thesis`; every other type -> `Other` |
| `title` | `title` through its base field (`caseName`, `subject`, ...) |
| `authors` | creators of the type's **primary** creator type, in order; a single-field creator becomes `family` = name, `given` = "" (kovan's organisation convention) |
| `abstract_text` | `abstractNote` |
| `year` | the year of `date` (through its base field), when non-negative |
| `doi` | `DOI`, else a `DOI: ...` line of Extra |
| `journal` | `publicationTitle` through its base field (so `bookTitle`, `websiteTitle`, `proceedingsTitle`, ...) |
| `institution` | for `report`/`thesis`: the `publisher`-mapped field (`institution`, `university`) |
| `publisher` | for every other type: `publisher` through its base field |
| `volume`, `pages` | through their base fields |
| `number` | `issue` when the type has one, else `number` through its base field |
| `keywords` | automatic tags (`type: 1`) |
| `tags` | manual tags |
| `source_url` | `url` |
| `source_path` | `path` of the first `linked_file` child in `attachments` (export format) |
| `page_count` | `numPages`, when it is a plain integer |
| `zotero_item` | the whole item, verbatim |

**Lost going Zotero -> kovan, except through `zotero_item`:** every
other field (`extra`, `language`, `series`, `ISSN`, `accessDate`, ...); the
full date (only the year is kept); non-primary creators (editors,
translators, ...); `key`/`version` beyond the id; collections, relations,
notes, attachments other than one path, annotations, `dateAdded`/
`dateModified`; and whether a `Paper` was a conference paper, a preprint
or a magazine article.

Because the item is kept in `zotero_item`, Zotero -> kovan -> Zotero is
lossless: [`ZoteroItem::from_kovan_document`] starts from the stored item
and only overwrites what kovan holds ~~(tested)~~. **CORRECTED
2026-10-07 (#752):** it was lossless only for items carrying a
`citationKey` and no stored file, which is all the `itemJSON` fixtures
tested. An item without `citationKey` came back with its derived slug
as a new `citationKey`, and a `source_path` pointing at a stored
attachment's file (`storage/<KEY>/<filename>`, as the local-library
import sets it) came back as an extra linked-file attachment. Both are
fixed: the slug is written only when it differs from the one the stored
item gives, and a stored attachment's own file is recognised. Tested
here and end to end in kovan's `tests/zotero_cli.rs`.

## kovan -> Zotero ([`ZoteroItem::from_kovan_document`])

The reverse of the table, starting from `zotero_item` when present (its
item type is kept). Kovan values whose field the item type lacks go into
Extra (`Publication Title: ...`); a `DOI:` Extra line is read back.

**Lost going kovan -> Zotero -> kovan:** `id` unless it is `zotero:<key>`;
`visibility` (comes back `Proprietary`); `Author::affiliation`;
`source_sha256`, `assets`, `related_symbols`, `related_repositories`,
`related_benchmarks`, `markdown_body`; `page_count` for types without
`numPages`; the `document_type` distinctions Zotero lacks (`Benchmark`
becomes `report` -> `Report`, `Manual` becomes `document` -> `Other`);
`journal` on a type with no publication-title field (it is written to
Extra, not read back); and `publisher` on `report`/`thesis`, whose one
publisher-mapped field holds `institution` (publisher goes to Extra).

```rust
pub mod kovan { /* ... */ }
```

### Functions

#### Function `document_type_for`

The kovan document type of a Zotero item type.

```rust
pub fn document_type_for(item_type: super::schema_generated::ItemType) -> crate::document::DocumentType { /* ... */ }
```

#### Function `item_type_for`

The Zotero item type for a kovan document type (a new item's type).

```rust
pub fn item_type_for(document_type: crate::document::DocumentType) -> super::schema_generated::ItemType { /* ... */ }
```

### Constants and Statics

#### Constant `ZOTERO_ID_PREFIX`

The id prefix of a document imported from Zotero.

```rust
pub const ZOTERO_ID_PREFIX: &str = "zotero:";
```

## Module `schema`

Schema queries: the port of `Zotero.ItemFields`, `Zotero.ItemTypes` and
`Zotero.CreatorTypes` over the generated tables.

Zotero keys these by database ids; the port keys them by the generated
enums instead (the ids are an artefact of a local SQLite database and are
not part of the JSON formats). Where an upstream function takes "an id or a
name", the port takes the enum and offers `from_name` for the string.

```rust
pub mod schema { /* ... */ }
```

### Types

#### Struct `ItemTypeField`

One field of an item type, with the base field it maps to.

`websiteTitle` on `webpage` has `base_field = Some(publicationTitle)`:
Zotero stores the value under the type-specific name and reads it back
through the base name (see [`field_from_type_and_base`]).

```rust
pub struct ItemTypeField {
    pub field: super::schema_generated::Field,
    pub base_field: Option<super::schema_generated::Field>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `super::schema_generated::Field` | The field as it appears in this item type's JSON. |
| `base_field` | `Option<super::schema_generated::Field>` | The base field it is mapped to, if any. |

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
    fn clone(self: &Self) -> ItemTypeField { /* ... */ }
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
    fn eq(self: &Self, other: &ItemTypeField) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `ItemTypeSchema`

The schema of one item type: its fields in display order, its creator
types and its primary creator type.

```rust
pub struct ItemTypeSchema {
    pub item_type: super::schema_generated::ItemType,
    pub fields: &'static [ItemTypeField],
    pub creator_types: &'static [super::schema_generated::CreatorType],
    pub primary_creator_type: Option<super::schema_generated::CreatorType>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `item_type` | `super::schema_generated::ItemType` | The item type. |
| `fields` | `&'static [ItemTypeField]` | Its fields, in schema (display) order. |
| `creator_types` | `&'static [super::schema_generated::CreatorType]` | Its creator types, in schema order. |
| `primary_creator_type` | `Option<super::schema_generated::CreatorType>` | The creator type marked `primary` (`None` for note, attachment and<br>annotation, which take no creators). |

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
    fn clone(self: &Self) -> ItemTypeSchema { /* ... */ }
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
    fn eq(self: &Self, other: &ItemTypeSchema) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `is_valid_for_type`

Whether `field` is valid for `item_type`
(`Zotero.ItemFields.isValidForType`, itemFields.js:170).

```rust
pub fn is_valid_for_type(field: super::schema_generated::Field, item_type: super::schema_generated::ItemType) -> bool { /* ... */ }
```

#### Function `field_from_type_and_base`

The type-specific field of `item_type` for `base_field`
(`Zotero.ItemFields.getFieldIDFromTypeAndBase`, itemFields.js:278).

* `webpage` + `publicationTitle` -> `websiteTitle`;
* `book` + `publisher` -> `publisher` (a base field valid for the type
  maps to itself, itemFields.js:552);
* `audioRecording` + `number` -> `None`;
* a field that is not a base field is returned when valid for the type
  (itemFields.js:290).

```rust
pub fn field_from_type_and_base(item_type: super::schema_generated::ItemType, base_field: super::schema_generated::Field) -> Option<super::schema_generated::Field> { /* ... */ }
```

#### Function `base_from_type_and_field`

The base field of `field` within `item_type`
(`Zotero.ItemFields.getBaseIDFromTypeAndField`, itemFields.js:311).

A base field valid for the type maps to itself; `audioRecording` +
`label` -> `publisher`; a field with no mapping -> `None`.

```rust
pub fn base_from_type_and_field(item_type: super::schema_generated::ItemType, field: super::schema_generated::Field) -> Option<super::schema_generated::Field> { /* ... */ }
```

#### Function `type_fields_from_base`

Every type-specific field mapped to `base_field`, across all item types,
without duplicates, in schema order (`Zotero.ItemFields.getTypeFieldsFromBase`,
itemFields.js:336). Empty when `base_field` is not a base field (upstream
returns `false`).

```rust
pub fn type_fields_from_base(base_field: super::schema_generated::Field) -> Vec<super::schema_generated::Field> { /* ... */ }
```

#### Function `is_valid_creator_type`

Whether `creator_type` is valid for `item_type`
(`Zotero.CreatorTypes.isValidForItemType`, cachedTypes.js:288).

```rust
pub fn is_valid_creator_type(creator_type: super::schema_generated::CreatorType, item_type: super::schema_generated::ItemType) -> bool { /* ... */ }
```

#### Function `primary_creator_type`

The primary creator type of `item_type`
(`Zotero.CreatorTypes.getPrimaryIDForType`).

```rust
pub fn primary_creator_type(item_type: super::schema_generated::ItemType) -> Option<super::schema_generated::CreatorType> { /* ... */ }
```

#### Function `item_types_for_csl_type`

The Zotero item types a CSL type imports as, first being the default
(`Zotero.Schema.CSL_TYPE_MAPPINGS_REVERSE`, utilities schema.js:43).

```rust
pub fn item_types_for_csl_type(csl_type: &str) -> Option<&'static [super::schema_generated::ItemType]> { /* ... */ }
```

#### Function `csl_variable_for_field`

The CSL variable a Zotero field maps to
(`Zotero.Schema.CSL_FIELD_MAPPINGS_REVERSE`, utilities schema.js:48-57):
text variables first, then date variables, a later mapping overwriting an
earlier one exactly as upstream's object assignment does.

```rust
pub fn csl_variable_for_field(field: super::schema_generated::Field) -> Option<&'static str> { /* ... */ }
```

## Module `search`

A Zotero saved search, in the JSON shape of `Zotero.Search#toJSON`
(search.js:893): `{key, version, name, conditions: [{condition, operator,
value}], deleted?}`.

The conditions are **kept as stored**, not evaluated ~~: running a saved
search needs Zotero's search engine (search.js `_buildQuery`, ~1500 lines
of SQL generation) and is not part of this port~~. **CORRECTED
2026-10-07 (#751)**: the search engine is ported, in memory, as
`kovan_discovery::zotero`; its `SearchLibrary::new` reads these stored
searches and `run_saved_search` evaluates one. This crate still only
stores them. A condition's
`condition` string carries its mode after a slash (`"title/any"`), exactly
as `toJSON` writes it.

Added 2026-10-07 for the Zotero database reader (#750); purely additive to
the #748 model.

```rust
pub mod search { /* ... */ }
```

### Types

#### Struct `SearchCondition`

One condition of a saved search, as `toJSON` writes it.

```rust
pub struct SearchCondition {
    pub condition: String,
    pub operator: Option<String>,
    pub value: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `condition` | `String` | `condition[/mode]`, e.g. `title`, `collection`, `fulltextContent/phraseBinary`. |
| `operator` | `Option<String>` | The operator, e.g. `is`, `contains`, `isNot`; `None` when stored as<br>NULL (upstream writes it through as `null`). |
| `value` | `String` | The value; `""` when none (`toJSON` writes `value || ""`). |

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
    fn clone(self: &Self) -> SearchCondition { /* ... */ }
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
    fn eq(self: &Self, other: &SearchCondition) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Struct `ZoteroSearch`

A Zotero saved search.

```rust
pub struct ZoteroSearch {
    pub key: Option<String>,
    pub version: Option<u64>,
    pub name: String,
    pub conditions: Vec<SearchCondition>,
    pub deleted: Option<bool>,
    pub other: std::collections::BTreeMap<String, serde_json::Value>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `key` | `Option<String>` | The 8-character object key. |
| `version` | `Option<u64>` | The object version. |
| `name` | `String` | The search's name. |
| `conditions` | `Vec<SearchCondition>` | The conditions, in order. |
| `deleted` | `Option<bool>` | `deleted` (in the trash); written only when true, as `_postToJSON` does. |
| `other` | `std::collections::BTreeMap<String, serde_json::Value>` | Every other property, verbatim. |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(name: impl Into<String>) -> Self { /* ... */ }
  ```
  A search with this name and no conditions.

- ```rust
  pub fn from_json_value(v: &Value) -> Result<Self, ZoteroJsonError> { /* ... */ }
  ```
  Parse search JSON (lenient: unknown properties go to `other`).

- ```rust
  pub fn to_json_value(self: &Self) -> Value { /* ... */ }
  ```
  Write search JSON as `toJSON` does.

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
    fn clone(self: &Self) -> ZoteroSearch { /* ... */ }
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
    fn deserialize<D: serde::Deserializer<''de>>(d: D) -> Result<Self, <D as >::Error> { /* ... */ }
    ```

- **DeserializeOwned**
- **Eq**
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
    fn eq(self: &Self, other: &ZoteroSearch) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Serialize**
  - ```rust
    fn serialize<S: serde::Serializer>(self: &Self, s: S) -> Result<<S as >::Ok, <S as >::Error> { /* ... */ }
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
## Module `schema_generated`

**Attributes:**

- `Other("#[rustfmt::skip]")`

```rust
pub mod schema_generated { /* ... */ }
```

### Types

#### Enum `ItemType`

A Zotero item type (schema.json `itemTypes[].itemType`).

Serialises as Zotero's own name (`annotation`), via [`ItemType::as_str`].

```rust
pub enum ItemType {
    Annotation,
    Artwork,
    Attachment,
    AudioRecording,
    Bill,
    BlogPost,
    Book,
    BookSection,
    Case,
    ComputerProgram,
    ConferencePaper,
    Dataset,
    DictionaryEntry,
    Document,
    Email,
    EncyclopediaArticle,
    Film,
    ForumPost,
    Hearing,
    InstantMessage,
    Interview,
    JournalArticle,
    Letter,
    MagazineArticle,
    Manuscript,
    Map,
    NewspaperArticle,
    Note,
    Patent,
    Podcast,
    Preprint,
    Presentation,
    RadioBroadcast,
    Report,
    Standard,
    Statute,
    Thesis,
    TvBroadcast,
    VideoRecording,
    Webpage,
}
```

##### Variants

###### `Annotation`

`annotation`

###### `Artwork`

`artwork`

###### `Attachment`

`attachment`

###### `AudioRecording`

`audioRecording`

###### `Bill`

`bill`

###### `BlogPost`

`blogPost`

###### `Book`

`book`

###### `BookSection`

`bookSection`

###### `Case`

`case`

###### `ComputerProgram`

`computerProgram`

###### `ConferencePaper`

`conferencePaper`

###### `Dataset`

`dataset`

###### `DictionaryEntry`

`dictionaryEntry`

###### `Document`

`document`

###### `Email`

`email`

###### `EncyclopediaArticle`

`encyclopediaArticle`

###### `Film`

`film`

###### `ForumPost`

`forumPost`

###### `Hearing`

`hearing`

###### `InstantMessage`

`instantMessage`

###### `Interview`

`interview`

###### `JournalArticle`

`journalArticle`

###### `Letter`

`letter`

###### `MagazineArticle`

`magazineArticle`

###### `Manuscript`

`manuscript`

###### `Map`

`map`

###### `NewspaperArticle`

`newspaperArticle`

###### `Note`

`note`

###### `Patent`

`patent`

###### `Podcast`

`podcast`

###### `Preprint`

`preprint`

###### `Presentation`

`presentation`

###### `RadioBroadcast`

`radioBroadcast`

###### `Report`

`report`

###### `Standard`

`standard`

###### `Statute`

`statute`

###### `Thesis`

`thesis`

###### `TvBroadcast`

`tvBroadcast`

###### `VideoRecording`

`videoRecording`

###### `Webpage`

`webpage`

##### Implementations

###### Methods

- ```rust
  pub fn schema(self: Self) -> &'static ItemTypeSchema { /* ... */ }
  ```
  The schema entry of this item type.

- ```rust
  pub fn fields(self: Self) -> impl Iterator<Item = Field> { /* ... */ }
  ```
  The fields valid for this item type, in display order

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  The en-US label, e.g. "Journal Article".

- ```rust
  pub fn csl_type(self: Self) -> Option<&'static str> { /* ... */ }
  ```
  The CSL type this item type exports as (`Zotero.Schema.CSL_TYPE_MAPPINGS`,

- ```rust
  pub fn is_regular(self: Self) -> bool { /* ... */ }
  ```
  Whether this is a "regular" item (not a note, attachment or annotation).

- ```rust
  pub const fn as_str(self: Self) -> &'static str { /* ... */ }
  ```
  Zotero's name for this ItemType.

- ```rust
  pub fn from_name(name: &str) -> Option<ItemType> { /* ... */ }
  ```
  The ItemType Zotero calls `name` (exact, case-sensitive match).

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
    fn clone(self: &Self) -> ItemType { /* ... */ }
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &ItemType) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &ItemType) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &ItemType) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Enum `Field`

A Zotero item field, in Zotero's registration order (each type's `field`, then its `baseField`).

Serialises as Zotero's own name (`title`), via [`Field::as_str`].

```rust
pub enum Field {
    Title,
    AbstractNote,
    ArtworkMedium,
    Medium,
    ArtworkSize,
    Date,
    EventPlace,
    Doi,
    CitationKey,
    Url,
    AccessDate,
    Archive,
    ArchiveLocation,
    ShortTitle,
    Language,
    LibraryCatalog,
    CallNumber,
    Rights,
    Extra,
    AudioRecordingFormat,
    SeriesTitle,
    Volume,
    NumberOfVolumes,
    Label,
    Publisher,
    Place,
    RunningTime,
    Isbn,
    BillNumber,
    Number,
    Code,
    CodeVolume,
    Section,
    CodePages,
    Pages,
    LegislativeBody,
    Authority,
    Session,
    History,
    BlogTitle,
    PublicationTitle,
    WebsiteType,
    Type,
    Issn,
    Series,
    SeriesNumber,
    Edition,
    OriginalDate,
    OriginalPublisher,
    OriginalPlace,
    Format,
    NumPages,
    BookTitle,
    CaseName,
    Court,
    DateDecided,
    DocketNumber,
    Reporter,
    ReporterVolume,
    FirstPage,
    VersionNumber,
    System,
    Company,
    ProgrammingLanguage,
    ProceedingsTitle,
    ConferenceName,
    Issue,
    Identifier,
    Repository,
    RepositoryLocation,
    DictionaryTitle,
    Subject,
    EncyclopediaTitle,
    Distributor,
    Genre,
    VideoRecordingFormat,
    ForumTitle,
    PostType,
    Committee,
    DocumentNumber,
    InterviewMedium,
    PartNumber,
    PartTitle,
    SeriesText,
    JournalAbbreviation,
    Pmid,
    Pmcid,
    LetterType,
    ManuscriptType,
    Institution,
    MapType,
    Scale,
    Country,
    Assignee,
    IssuingAuthority,
    PatentNumber,
    FilingDate,
    ApplicationNumber,
    PriorityNumbers,
    IssueDate,
    PriorityDate,
    References,
    LegalStatus,
    Status,
    EpisodeNumber,
    AudioFileType,
    ArchiveID,
    PresentationType,
    MeetingName,
    SessionTitle,
    ProgramTitle,
    Network,
    ReportNumber,
    ReportType,
    Organization,
    NameOfAct,
    CodeNumber,
    PublicLawNumber,
    DateEnacted,
    ThesisType,
    University,
    Studio,
    WebsiteTitle,
}
```

##### Variants

###### `Title`

`title`

###### `AbstractNote`

`abstractNote`

###### `ArtworkMedium`

`artworkMedium`

###### `Medium`

`medium`

###### `ArtworkSize`

`artworkSize`

###### `Date`

`date`

###### `EventPlace`

`eventPlace`

###### `Doi`

`DOI`

###### `CitationKey`

`citationKey`

###### `Url`

`url`

###### `AccessDate`

`accessDate`

###### `Archive`

`archive`

###### `ArchiveLocation`

`archiveLocation`

###### `ShortTitle`

`shortTitle`

###### `Language`

`language`

###### `LibraryCatalog`

`libraryCatalog`

###### `CallNumber`

`callNumber`

###### `Rights`

`rights`

###### `Extra`

`extra`

###### `AudioRecordingFormat`

`audioRecordingFormat`

###### `SeriesTitle`

`seriesTitle`

###### `Volume`

`volume`

###### `NumberOfVolumes`

`numberOfVolumes`

###### `Label`

`label`

###### `Publisher`

`publisher`

###### `Place`

`place`

###### `RunningTime`

`runningTime`

###### `Isbn`

`ISBN`

###### `BillNumber`

`billNumber`

###### `Number`

`number`

###### `Code`

`code`

###### `CodeVolume`

`codeVolume`

###### `Section`

`section`

###### `CodePages`

`codePages`

###### `Pages`

`pages`

###### `LegislativeBody`

`legislativeBody`

###### `Authority`

`authority`

###### `Session`

`session`

###### `History`

`history`

###### `BlogTitle`

`blogTitle`

###### `PublicationTitle`

`publicationTitle`

###### `WebsiteType`

`websiteType`

###### `Type`

`type`

###### `Issn`

`ISSN`

###### `Series`

`series`

###### `SeriesNumber`

`seriesNumber`

###### `Edition`

`edition`

###### `OriginalDate`

`originalDate`

###### `OriginalPublisher`

`originalPublisher`

###### `OriginalPlace`

`originalPlace`

###### `Format`

`format`

###### `NumPages`

`numPages`

###### `BookTitle`

`bookTitle`

###### `CaseName`

`caseName`

###### `Court`

`court`

###### `DateDecided`

`dateDecided`

###### `DocketNumber`

`docketNumber`

###### `Reporter`

`reporter`

###### `ReporterVolume`

`reporterVolume`

###### `FirstPage`

`firstPage`

###### `VersionNumber`

`versionNumber`

###### `System`

`system`

###### `Company`

`company`

###### `ProgrammingLanguage`

`programmingLanguage`

###### `ProceedingsTitle`

`proceedingsTitle`

###### `ConferenceName`

`conferenceName`

###### `Issue`

`issue`

###### `Identifier`

`identifier`

###### `Repository`

`repository`

###### `RepositoryLocation`

`repositoryLocation`

###### `DictionaryTitle`

`dictionaryTitle`

###### `Subject`

`subject`

###### `EncyclopediaTitle`

`encyclopediaTitle`

###### `Distributor`

`distributor`

###### `Genre`

`genre`

###### `VideoRecordingFormat`

`videoRecordingFormat`

###### `ForumTitle`

`forumTitle`

###### `PostType`

`postType`

###### `Committee`

`committee`

###### `DocumentNumber`

`documentNumber`

###### `InterviewMedium`

`interviewMedium`

###### `PartNumber`

`partNumber`

###### `PartTitle`

`partTitle`

###### `SeriesText`

`seriesText`

###### `JournalAbbreviation`

`journalAbbreviation`

###### `Pmid`

`PMID`

###### `Pmcid`

`PMCID`

###### `LetterType`

`letterType`

###### `ManuscriptType`

`manuscriptType`

###### `Institution`

`institution`

###### `MapType`

`mapType`

###### `Scale`

`scale`

###### `Country`

`country`

###### `Assignee`

`assignee`

###### `IssuingAuthority`

`issuingAuthority`

###### `PatentNumber`

`patentNumber`

###### `FilingDate`

`filingDate`

###### `ApplicationNumber`

`applicationNumber`

###### `PriorityNumbers`

`priorityNumbers`

###### `IssueDate`

`issueDate`

###### `PriorityDate`

`priorityDate`

###### `References`

`references`

###### `LegalStatus`

`legalStatus`

###### `Status`

`status`

###### `EpisodeNumber`

`episodeNumber`

###### `AudioFileType`

`audioFileType`

###### `ArchiveID`

`archiveID`

###### `PresentationType`

`presentationType`

###### `MeetingName`

`meetingName`

###### `SessionTitle`

`sessionTitle`

###### `ProgramTitle`

`programTitle`

###### `Network`

`network`

###### `ReportNumber`

`reportNumber`

###### `ReportType`

`reportType`

###### `Organization`

`organization`

###### `NameOfAct`

`nameOfAct`

###### `CodeNumber`

`codeNumber`

###### `PublicLawNumber`

`publicLawNumber`

###### `DateEnacted`

`dateEnacted`

###### `ThesisType`

`thesisType`

###### `University`

`university`

###### `Studio`

`studio`

###### `WebsiteTitle`

`websiteTitle`

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  The en-US label, e.g. "Publication".

- ```rust
  pub fn is_base_field(self: Self) -> bool { /* ... */ }
  ```
  Whether this field is a base field of at least one item type

- ```rust
  pub fn is_date(self: Self) -> bool { /* ... */ }
  ```
  Whether this field holds a date (`Zotero.ItemFields.isDate`,

- ```rust
  pub const fn as_str(self: Self) -> &'static str { /* ... */ }
  ```
  Zotero's name for this Field.

- ```rust
  pub fn from_name(name: &str) -> Option<Field> { /* ... */ }
  ```
  The Field Zotero calls `name` (exact, case-sensitive match).

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
    fn clone(self: &Self) -> Field { /* ... */ }
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &Field) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Field) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &Field) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
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
#### Enum `CreatorType`

A Zotero creator type (schema.json `itemTypes[].creatorTypes[]`).

Serialises as Zotero's own name (`artist`), via [`CreatorType::as_str`].

```rust
pub enum CreatorType {
    Artist,
    Contributor,
    Performer,
    OriginalCreator,
    Composer,
    WordsBy,
    Translator,
    Sponsor,
    Cosponsor,
    Author,
    Commenter,
    Editor,
    SeriesEditor,
    BookAuthor,
    Counsel,
    Programmer,
    ReviewedAuthor,
    Recipient,
    Director,
    Producer,
    Scriptwriter,
    CastMember,
    Host,
    Guest,
    Narrator,
    Interviewee,
    Interviewer,
    Cartographer,
    Inventor,
    AttorneyAgent,
    Podcaster,
    ExecutiveProducer,
    SeriesCreator,
    Presenter,
    Chair,
    Organizer,
    Creator,
}
```

##### Variants

###### `Artist`

`artist`

###### `Contributor`

`contributor`

###### `Performer`

`performer`

###### `OriginalCreator`

`originalCreator`

###### `Composer`

`composer`

###### `WordsBy`

`wordsBy`

###### `Translator`

`translator`

###### `Sponsor`

`sponsor`

###### `Cosponsor`

`cosponsor`

###### `Author`

`author`

###### `Commenter`

`commenter`

###### `Editor`

`editor`

###### `SeriesEditor`

`seriesEditor`

###### `BookAuthor`

`bookAuthor`

###### `Counsel`

`counsel`

###### `Programmer`

`programmer`

###### `ReviewedAuthor`

`reviewedAuthor`

###### `Recipient`

`recipient`

###### `Director`

`director`

###### `Producer`

`producer`

###### `Scriptwriter`

`scriptwriter`

###### `CastMember`

`castMember`

###### `Host`

`host`

###### `Guest`

`guest`

###### `Narrator`

`narrator`

###### `Interviewee`

`interviewee`

###### `Interviewer`

`interviewer`

###### `Cartographer`

`cartographer`

###### `Inventor`

`inventor`

###### `AttorneyAgent`

`attorneyAgent`

###### `Podcaster`

`podcaster`

###### `ExecutiveProducer`

`executiveProducer`

###### `SeriesCreator`

`seriesCreator`

###### `Presenter`

`presenter`

###### `Chair`

`chair`

###### `Organizer`

`organizer`

###### `Creator`

`creator`

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  The en-US label, e.g. "Author".

- ```rust
  pub fn csl_name(self: Self) -> Option<&'static str> { /* ... */ }
  ```
  The CSL name variable this creator type maps to (`csl.names`), or

- ```rust
  pub const fn as_str(self: Self) -> &'static str { /* ... */ }
  ```
  Zotero's name for this CreatorType.

- ```rust
  pub fn from_name(name: &str) -> Option<CreatorType> { /* ... */ }
  ```
  The CreatorType Zotero calls `name` (exact, case-sensitive match).

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
    fn clone(self: &Self) -> CreatorType { /* ... */ }
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

- **Ord**
  - ```rust
    fn cmp(self: &Self, other: &CreatorType) -> $crate::cmp::Ordering { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &CreatorType) -> bool { /* ... */ }
    ```

- **PartialOrd**
  - ```rust
    fn partial_cmp(self: &Self, other: &CreatorType) -> $crate::option::Option<$crate::cmp::Ordering> { /* ... */ }
    ```

- **RefUnwindSafe**
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
### Constants and Statics

#### Constant `SCHEMA_VERSION`

The `version` of the schema.json these tables were generated from.

```rust
pub const SCHEMA_VERSION: u32 = 45;
```

#### Constant `SCHEMA_COMMIT`

The zotero-schema commit these tables were generated from.

```rust
pub const SCHEMA_COMMIT: &str = "b86c79b56479";
```

#### Static `ITEM_TYPE_SCHEMAS`

Every item type's fields (schema order, with base field) and creator types.

```rust
pub static ITEM_TYPE_SCHEMAS: [super::schema::ItemTypeSchema; 40] = _;
```

#### Static `META_FIELD_TYPES`

schema.json `meta.fields`: the value type of special fields (e.g. `"date"`).

```rust
pub static META_FIELD_TYPES: [(Field, &str); 3] = _;
```

#### Static `CSL_TYPES`

schema.json `csl.types`: CSL type -> Zotero item types (first is the import default).

```rust
pub static CSL_TYPES: [(&str, &[ItemType]); 32] = _;
```

#### Static `CSL_TEXT_FIELDS`

schema.json `csl.fields.text`: CSL variable -> Zotero fields, in priority order.

```rust
pub static CSL_TEXT_FIELDS: [(&str, &[Field]); 47] = _;
```

#### Static `CSL_DATE_FIELDS`

schema.json `csl.fields.date`: CSL date variable -> Zotero field.

```rust
pub static CSL_DATE_FIELDS: [(&str, Field); 4] = _;
```

#### Static `CSL_NAMES`

schema.json `csl.names`: Zotero creator type -> CSL name variable.

```rust
pub static CSL_NAMES: [(CreatorType, &str); 24] = _;
```

#### Static `EN_US_ITEM_TYPE_LABELS`

en-US item type labels (schema.json `locales.en-US.itemTypes`).

```rust
pub static EN_US_ITEM_TYPE_LABELS: [(ItemType, &str); 40] = _;
```

#### Static `EN_US_FIELD_LABELS`

en-US field labels (schema.json `locales.en-US.fields`), keyed by name because
the locale also labels the item properties `dateAdded`, `dateModified`, `itemType`.

```rust
pub static EN_US_FIELD_LABELS: [(&str, &str); 126] = _;
```

#### Static `EN_US_CREATOR_TYPE_LABELS`

en-US creator type labels (schema.json `locales.en-US.creatorTypes`).

```rust
pub static EN_US_CREATOR_TYPE_LABELS: [(CreatorType, &str); 37] = _;
```

## Module `validate`

Checking an item against the schema, and Zotero's import-time repair of
fields that do not belong to the item type.

```rust
pub mod validate { /* ... */ }
```

### Types

#### Enum `ValidationIssue`

One way an item does not match the schema.

```rust
pub enum ValidationIssue {
    UnknownField(String),
    FieldNotValidForType {
        field: super::schema_generated::Field,
        mapped: Option<super::schema_generated::Field>,
    },
    CreatorTypeNotValidForType(super::schema_generated::CreatorType),
    CreatorsOnNonRegularItem,
    AttachmentDataOnNonAttachment,
    AnnotationDataOnNonAnnotation,
    AnnotationWithoutParent,
}
```

##### Variants

###### `UnknownField`

A property name that is no Zotero field.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `String` |  |

###### `FieldNotValidForType`

A field that the item type does not have. `mapped` is the
type-specific field the value belongs in when `field` is a base field
with a mapping for this type (e.g. `publicationTitle` on a `webpage`
belongs in `websiteTitle`).

Fields:

| Name | Type | Documentation |
|------|------|---------------|
| `field` | `super::schema_generated::Field` | The field as written. |
| `mapped` | `Option<super::schema_generated::Field>` | The type-specific field to use instead, if any. |

###### `CreatorTypeNotValidForType`

A creator whose type is not valid for the item type.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `super::schema_generated::CreatorType` |  |

###### `CreatorsOnNonRegularItem`

Creators on a note, attachment or annotation.

###### `AttachmentDataOnNonAttachment`

Attachment properties on an item that is not an attachment.

###### `AnnotationDataOnNonAnnotation`

Annotation properties on an item that is not an annotation.

###### `AnnotationWithoutParent`

An annotation without `parentItem`.

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
    fn clone(self: &Self) -> ValidationIssue { /* ... */ }
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
    fn eq(self: &Self, other: &ValidationIssue) -> bool { /* ... */ }
    ```

- **RefUnwindSafe**
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

#### Function `camel_to_title_case`

`Zotero.Utilities.Internal.camelToTitleCase` (utilities_internal.js:1607):
`publicationTitle` -> `Publication Title`.

```rust
pub fn camel_to_title_case(s: &str) -> String { /* ... */ }
```

#### Function `combine_extra_fields`

`Zotero.Utilities.Internal.combineExtraFields` (utilities_internal.js:1387):
update `Key: value` lines of `extra` that name a field in `fields`, and
prepend the rest as sorted `Title Case: value` lines.

```rust
pub fn combine_extra_fields(extra: &str, fields: Vec<(String, String)>) -> String { /* ... */ }
```

### Re-exports

#### Re-export `item_from_csl_json`

```rust
pub use csl::item_from_csl_json;
```

#### Re-export `item_to_csl_json`

```rust
pub use csl::item_to_csl_json;
```

#### Re-export `CslError`

```rust
pub use csl::CslError;
```

#### Re-export `CslItem`

```rust
pub use csl::CslItem;
```

#### Re-export `AnnotationData`

```rust
pub use item::AnnotationData;
```

#### Re-export `AnnotationType`

```rust
pub use item::AnnotationType;
```

#### Re-export `AttachmentData`

```rust
pub use item::AttachmentData;
```

#### Re-export `Creator`

```rust
pub use item::Creator;
```

#### Re-export `CreatorName`

```rust
pub use item::CreatorName;
```

#### Re-export `LinkMode`

```rust
pub use item::LinkMode;
```

#### Re-export `Tag`

```rust
pub use item::Tag;
```

#### Re-export `ZoteroCollection`

```rust
pub use item::ZoteroCollection;
```

#### Re-export `ZoteroItem`

```rust
pub use item::ZoteroItem;
```

#### Re-export `ZoteroJsonError`

```rust
pub use item::ZoteroJsonError;
```

#### Re-export `ZoteroLibrary`

```rust
pub use item::ZoteroLibrary;
```

#### Re-export `ItemTypeField`

```rust
pub use schema::ItemTypeField;
```

#### Re-export `ItemTypeSchema`

```rust
pub use schema::ItemTypeSchema;
```

#### Re-export `SearchCondition`

```rust
pub use search::SearchCondition;
```

#### Re-export `ZoteroSearch`

```rust
pub use search::ZoteroSearch;
```

#### Re-export `CreatorType`

```rust
pub use schema_generated::CreatorType;
```

#### Re-export `Field`

```rust
pub use schema_generated::Field;
```

#### Re-export `ItemType`

```rust
pub use schema_generated::ItemType;
```

#### Re-export `SCHEMA_COMMIT`

```rust
pub use schema_generated::SCHEMA_COMMIT;
```

#### Re-export `SCHEMA_VERSION`

```rust
pub use schema_generated::SCHEMA_VERSION;
```

#### Re-export `ValidationIssue`

```rust
pub use validate::ValidationIssue;
```

## Re-exports

### Re-export `Author`

```rust
pub use document::Author;
```

### Re-export `DocumentType`

```rust
pub use document::DocumentType;
```

### Re-export `KovanDocument`

```rust
pub use document::KovanDocument;
```

### Re-export `KovanDocumentBuilder`

```rust
pub use document::KovanDocumentBuilder;
```

### Re-export `Visibility`

```rust
pub use document::Visibility;
```

### Re-export `GeneratedArtifact`

```rust
pub use knowledge::GeneratedArtifact;
```

### Re-export `KovanBenchmark`

```rust
pub use knowledge::KovanBenchmark;
```

### Re-export `KovanCorrelation`

```rust
pub use knowledge::KovanCorrelation;
```

### Re-export `KovanValidationCase`

```rust
pub use knowledge::KovanValidationCase;
```

### Re-export `KovanRepository`

```rust
pub use symbol::KovanRepository;
```

### Re-export `KovanSymbol`

```rust
pub use symbol::KovanSymbol;
```

### Re-export `Language`

```rust
pub use symbol::Language;
```

