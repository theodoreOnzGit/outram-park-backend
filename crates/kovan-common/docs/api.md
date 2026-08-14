# Crate Documentation

**Version:** 0.0.0

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
that lives in the respective feature crate.

## Module map

- [`document`] — [`KovanDocument`] + [`KovanDocumentBuilder`], [`Author`],
  [`Visibility`], [`DocumentType`].
- [`symbol`] — [`KovanSymbol`], [`KovanRepository`], the [`Language`] enum.
- [`knowledge`] — [`KovanCorrelation`], [`KovanBenchmark`],
  [`KovanValidationCase`], [`GeneratedArtifact`].

Everything is re-exported at the crate root, so downstream crates can keep
importing `kovan_common::KovanDocument` directly.

## Maturity

Unlike the other `kovan-*` crates, this one is **not** a placeholder stage
with stub logic — it is a plain data crate (types + serde derives + a
builder + convenience constructors) and there is nothing left here to stub
out. Every public type is fully implemented, documented, and round-trip
tested (`serde_json` and `toml`). The pipeline crates that build on top of
these types (`kovan-literature`, `kovan-semantics`, `kovan-codegen`) still
carry their own `// TODO(kovan)` markers for unimplemented behaviour; that
is expected and tracked separately in each of those crates.

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

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

