# Crate Documentation

**Version:** 0.0.0

**Format Version:** 61

# Module `kovan_tui`

# kovan-tui

The **human-facing** entry point to KOVAN: a terminal UI for browsing
literature, repositories, and generated knowledge. Agents should use the
`kovan` CLI instead (see `kovan-cli`).

Built on [`ratatui`]. TUI is desktop scope — on Android the binary compiles
to a stub that redirects to the CLI, keeping the workspace Android-buildable
(see the root `CLAUDE.md` "Android portability" rule). The entire [`tui`]
module tree lives behind `cfg(not(target_os = "android"))` on the single
`mod tui;` declaration below, so none of its submodules need to repeat the
gate.

## Screens

Five tabs, switched with `1`-`5` or `Tab`/`Shift+Tab`:

1. **Overview** — static module map (the original placeholder screen).
2. **Browser** (`kovan_discovery`) — walk a repository root, filter by
   [`kovan_discovery::FileKind`], and navigate the discovered files.
3. **Symbols** (`kovan_semantics`) — catalogue a repository's symbols with
   the ripgrep-first extractor and preview either the raw list or the
   generated `symbols.md` Markdown artifact.
4. **Methods** (`kovan_codegen`) — browse the numerical-method catalogue by
   family and preview a method's generated source.
5. **Literature** (`kovan_literature`) — list PDFs / Markdown / BibTeX under
   a literature root and preview each one (metadata extraction, heading
   outline, or raw text).

Every screen is a **viewer only** — it reads the filesystem and renders
deterministic output from the sibling `kovan-*` crates; it never writes to
the repositories it browses (KOVAN's "not a repository modification agent"
non-goal, `docs/kovan.md` § "Non-Goals").

## Modules

## Module `tui`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/kovan-tui/src/main.rs:46:11: 46:32 (#0) }, crates/kovan-tui/src/main.rs:46:10: 46:33 (#0))])]")`

The desktop TUI application: terminal setup/teardown, the top-level
[`App`] state machine, and screen dispatch. This whole module tree is
compiled only when `main.rs` includes it (behind
`cfg(not(target_os = "android"))`), so nothing below needs to repeat that
gate.

# Navigation

Six tabs ([`Tab`]), switched with `1`-`6` or `Tab`/`Shift+Tab` whenever no
text field is being edited. Each tab that reads the filesystem (Browser,
Symbols, Literature, Ingest) owns a small text field for its root path,
entered with `e` and confirmed with `Enter`/cancelled with `Esc` — see
[`App::editing`]. `q`/`Esc` quits from any tab, except while editing (where
`Esc` only cancels the edit) and except when the Ingest tab has work in
flight (see [`App::handle_key`]).

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
pub(crate) mod tui { /* ... */ }
```

### Modules

## Module `browser`

Repository Browser tab — a human-facing view over `kovan_discovery`.

Type a root directory, cycle the [`FileKind`] filter (or "all"), and press
Enter to walk the tree with the same `.gitignore`-aware discovery the
`kovan discover` CLI subcommand uses (`kovan_discovery::discover` /
`discover_kind`). This tab only *reads* the filesystem.

```rust
pub(in ::tui) mod browser { /* ... */ }
```

### Types

#### Enum `KindFilter`

The active discovery filter: every [`FileKind`], plus an "all files" option
the library enum itself doesn't model (that's [`discover`] with no
extension filter, vs. [`discover_kind`] for a specific kind).

```rust
pub enum KindFilter {
    All,
    Kind(kovan_discovery::FileKind),
}
```

##### Variants

###### `All`

###### `Kind`

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `kovan_discovery::FileKind` |  |

##### Implementations

###### Methods

- ```rust
  pub(in ::tui::browser) fn label(self: Self) -> &'static str { /* ... */ }
  ```

- ```rust
  pub(in ::tui::browser) fn step(self: Self, delta: i32) -> Self { /* ... */ }
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
    fn clone(self: &Self) -> KindFilter { /* ... */ }
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
    fn eq(self: &Self, other: &KindFilter) -> bool { /* ... */ }
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
#### Struct `BrowserState`

State for the Repository Browser tab.

```rust
pub struct BrowserState {
    pub root: super::text_input::TextInput,
    pub kind: KindFilter,
    pub files: Vec<std::path::PathBuf>,
    pub list_state: ratatui::widgets::ListState,
    pub status: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `root` | `super::text_input::TextInput` |  |
| `kind` | `KindFilter` |  |
| `files` | `Vec<std::path::PathBuf>` |  |
| `list_state` | `ratatui::widgets::ListState` |  |
| `status` | `String` |  |

##### Implementations

###### Methods

- ```rust
  pub(in ::tui::browser) fn run_discovery(self: &mut Self) { /* ... */ }
  ```
  Re-run discovery against the current root/filter. Never panics on a

- ```rust
  pub(in ::tui::browser) fn select_next(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub(in ::tui::browser) fn select_prev(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub fn handle_key(self: &mut Self, key: KeyEvent, editing: &mut bool) { /* ... */ }
  ```
  Handle one key event. `editing` is the shared edit-mode flag owned by

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn default() -> Self { /* ... */ }
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

- **Read**
- **ReadPrimitive**
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
### Functions

#### Function `draw`

Render the Browser tab into `area`.

```rust
pub fn draw(frame: &mut ratatui::Frame<''_>, area: ratatui::layout::Rect, state: &mut BrowserState, editing: bool) { /* ... */ }
```

## Module `ingest`

Ingest tab — interactive literature ingestion (`kovan-literature`).

This is the TUI equivalent of `kovan lit import <PDF> [--json-out <p>]
[--markdown-out <p>]`, and it calls exactly the same library entry points the
CLI does ([`kovan_literature::extract_metadata`] and
[`kovan_literature::to_bibtex`]) — no reimplementation of the pipeline.

# The flow

```text
Picking ──Enter──▶ Running ──ok──▶ Review ──s──▶ (files written)
   ▲                  │              │
   └──────x───────────┴──error──▶ Failed ──x──┘
```

1. **Picking** — browse a directory for PDFs (the same `.gitignore`-aware
   walk the Browser tab uses, `kovan_discovery::discover_kind`), narrowed by
   a substring filter, so no absolute path has to be typed by hand.
2. **Running** — extraction runs on a worker thread; the draw loop keeps
   running and shows elapsed time. See "Why a thread" below.
3. **Review** — the extracted metadata is shown as an editable form with
   advisories, because `extract_metadata` is best-effort (see
   [`review`]'s module docs for the real-world failure that motivated it).
4. **Save** — writes Markdown, `KovanDocument` JSON, and BibTeX to the chosen
   paths, from the **corrected** record.

# Why a thread (and why a channel is right here)

`extract_metadata` is one opaque, unbounded, blocking call over a
user-supplied file. Running it on the draw-loop thread would freeze the UI
for however long it takes, with no indication that anything is happening —
exactly the failure this tab exists to avoid. So the call is moved to a
worker thread and its one result comes back over a `std::sync::mpsc` channel.

Measured cost (release build, developer desktop, 2026-08-05): a 12 MB /
447-page scanned report extracted in **0.3 s**, a 1.4 MB / 103-page one in
0.1 s. Faster than assumed — but the cost is a property of the file and the
machine (a debug build, an Android device, or a pathological PDF are all far
slower), and it is unbounded in principle, so the UI must not depend on it
being quick.

The workspace rule "no channels for simulation state, use `Arc<RwLock<T>>`"
(root `CLAUDE.md`, "Shared state") is about threads computing over *shared
mutable* fields in a timestep loop. This is the other pattern that rule
contrasts with: a produce-once/consume-once pipeline with no shared state at
all — the worker owns its `PathBuf`, the UI owns the result, and nothing is
ever mutated from two threads. A lock here would add ceremony and no safety.

Honesty about progress: `kovan-literature` exposes no progress callback, so
this tab shows **elapsed time and a liveness spinner, never a fabricated
percentage**.

# Robustness

- The worker wraps the library call in [`std::panic::catch_unwind`], so a
  panic inside PDF parsing becomes a normal `Failed` phase rather than
  unwinding the process and leaving the terminal in raw mode.
- A worker that dies without sending (channel disconnect) is reported too.
- Every save error is reported in-pane; nothing here panics or `unwrap`s on
  user-supplied paths.

This is the one tab that **writes** files, and only when the user presses
`s` on paths they can see and edit.

```rust
pub(in ::tui) mod ingest { /* ... */ }
```

### Modules

## Module `draw`

Rendering for the Ingest tab — one `draw_*` function per
[`IngestPhase`] variant.

Kept separate from the state machine in `mod.rs` so the reducer stays
testable without a terminal (the workspace's existing `kovan-tui` testing
approach) and neither file grows past the workspace file-size cap.

```rust
pub(in ::tui::ingest) mod draw { /* ... */ }
```

### Functions

#### Function `draw`

Render the Ingest tab into `area`.

`editing` is [`super::super::App`]'s shared edit-mode flag; it only affects
how the focused field is highlighted.

```rust
pub fn draw(frame: &mut ratatui::Frame<''_>, area: ratatui::layout::Rect, state: &mut super::IngestState, editing: bool) { /* ... */ }
```

#### Function `draw_picker`

**Attributes:**

- `Other("#[allow(clippy::too_many_arguments)]")`

The PDF picker: root directory, substring filter, and the discovered files.

```rust
pub(in ::tui::ingest::draw) fn draw_picker(frame: &mut ratatui::Frame<''_>, area: ratatui::layout::Rect, root: &str, filter: &str, field: super::PickerField, editing: bool, candidates: &[std::path::PathBuf], list_state: &mut ratatui::widgets::ListState) { /* ... */ }
```

#### Function `draw_running`

The progress screen. Shows elapsed time and a spinner — deliberately **not**
a percentage: `kovan-literature` exposes no progress callback, so any
percentage would be invented.

```rust
pub(in ::tui::ingest::draw) fn draw_running(frame: &mut ratatui::Frame<''_>, area: ratatui::layout::Rect, job: &super::RunningJob) { /* ... */ }
```

#### Function `draw_review`

The review form (left) beside the derived record and save report (right).

```rust
pub(in ::tui::ingest::draw) fn draw_review(frame: &mut ratatui::Frame<''_>, area: ratatui::layout::Rect, review: &super::review::ReviewState, editing: bool) { /* ... */ }
```

#### Function `derived_record_text`

The text of the "record to be saved" pane: provenance, the derived
identifiers, and the BibTeX that would be generated.

```rust
pub(in ::tui::ingest::draw) fn derived_record_text(review: &super::review::ReviewState) -> String { /* ... */ }
```

#### Function `draw_failed`

The failure screen. Extraction errors are shown here rather than aborting the
program, so the terminal is never left in raw mode by an ingestion problem.

```rust
pub(in ::tui::ingest::draw) fn draw_failed(frame: &mut ratatui::Frame<''_>, area: ratatui::layout::Rect, failure: &super::FailureReport) { /* ... */ }
```

### Constants and Statics

#### Constant `REVIEW_ROWS`

Rows of the review form, in display order. Mirrors the navigation order in
[`ReviewField::step`].

```rust
pub(in ::tui::ingest::draw) const REVIEW_ROWS: [super::review::ReviewField; 8] = _;
```

## Module `metadata`

Pure metadata helpers used by the review form: author-line parsing, the
year/report-number heuristics behind the advisories, and slug/id derivation.

Split out of [`super::review`] so each file stays inside the workspace's
file-size cap, and because everything here is a pure function of its inputs —
no state, no I/O — which makes it the easiest part of the ingestion flow to
test and to audit.

# Duplication flagged on purpose

[`make_slug`] and [`make_id`] **mirror private functions of the same name in
`kovan-literature`** (`src/metadata.rs`). That crate derives a document's
slug and id exactly once, inside `extract_metadata`, and exposes no public
way to re-derive them — so correcting a wrong year in the TUI would otherwise
leave the citation key frozen at the extractor's mistake (`2004anl7416`).
They are kept byte-for-byte compatible here, and the shared cases are pinned
by a test (`slug_matches_the_library_algorithm_on_a_known_case`). The proper
fix is a public `kovan_literature::derive_identifiers(&mut KovanDocument)`;
see this crate's `DECISIONS.md`.

```rust
pub(in ::tui::ingest) mod metadata { /* ... */ }
```

### Functions

#### Function `parse_authors`

Parse the author line into [`Author`]s.

Authors are separated by `;`. Within one author, a comma splits
`Family, Given` (BibTeX name order). An entry with no comma is treated as a
**corporate author**: the whole string becomes `family` with an empty
`given`, which is the convention `kovan_common::Author` documents for
organisations — so typing `Argonne Code Center` yields exactly one corporate
author, not three people.

Blank entries are dropped, so a trailing `;` is harmless.

```rust
pub fn parse_authors(text: &str) -> Vec<kovan_common::Author> { /* ... */ }
```

#### Function `format_authors`

Render an author list back into the editable form [`parse_authors`] accepts.
Round-trips: `parse_authors(&format_authors(&a)) == a` for any list built by
`parse_authors`.

```rust
pub fn format_authors(authors: &[kovan_common::Author]) -> String { /* ... */ }
```

#### Function `years_in_text`

Distinct plausible publication years appearing in `text`, ascending.

Scans only the first [`YEAR_SCAN_CHARS`] characters (a report's front matter)
and accepts any 4-digit run inside [`YEAR_RANGE`] that is not part of a longer
digit run — so `ANL-7416` and `19770` contribute nothing.

```rust
pub fn years_in_text(text: &str) -> Vec<u32> { /* ... */ }
```

#### Function `looks_like_report_number`

Heuristic: does `title` look like a bare report identifier (e.g.
`ANL-7416 Supplement 2`) rather than a descriptive title?

True when the string is short (at most four whitespace tokens) and contains a
token that mixes letters with digits or hyphens — the shape of a report
number. Advisory only; it never changes the data.

```rust
pub fn looks_like_report_number(title: &str) -> bool { /* ... */ }
```

#### Function `make_slug`

Build the BibTeX-style slug `<firstauthorfamily><year><firsttitleword>`,
lowercased and alphanumeric-only (e.g. `argonnecodecenter1977anl7416`),
falling back to a slugged title when neither author nor year is known.

**Mirrors `kovan-literature`'s private `make_slug`** (see the module docs and
this crate's `DECISIONS.md`): that crate derives the slug once, inside
`extract_metadata`, and exposes no way to re-derive it after a correction.
Kept byte-for-byte compatible so a document corrected here carries the same
slug the library would have produced had extraction been right first time.

```rust
pub fn make_slug(authors: &[kovan_common::Author], year: Option<u32>, title: &str) -> String { /* ... */ }
```

#### Function `slug_token`

Lowercase a token, keeping only ASCII alphanumerics.

```rust
pub(in ::tui::ingest::metadata) fn slug_token(s: &str) -> String { /* ... */ }
```

#### Function `make_id`

Build the stable content id `kovan-<fnv1a64 hex>` from slug and title.

Mirrors `kovan-literature`'s private `make_id` for the same reason as
[`make_slug`]. Deterministic — no timestamps, no randomness — so re-ingesting
a document with the same corrections yields the same id.

```rust
pub fn make_id(slug: &str, title: &str) -> String { /* ... */ }
```

#### Function `fnv1a64`

64-bit FNV-1a hash — small, dependency-free, deterministic. Used only for
document ids, never for security.

```rust
pub(in ::tui::ingest::metadata) fn fnv1a64(data: &[u8]) -> u64 { /* ... */ }
```

### Constants and Statics

#### Constant `YEAR_RANGE`

Plausible publication-year window used both for validating a typed year and
for scanning the document body for candidate years. Deliberately wide — the
point is to reject typos (`19777`, `abc`), not to second-guess the user.

```rust
pub const YEAR_RANGE: std::ops::RangeInclusive<u32> = _;
```

#### Constant `YEAR_SCAN_CHARS`

How much of the generated Markdown body is scanned for candidate publication
years (characters). The front matter of a report carries its real date; a
full 447-page scan would only add noise (every "in 1953 …" in the text).

```rust
pub(in ::tui::ingest::metadata) const YEAR_SCAN_CHARS: usize = 20_000;
```

## Module `review`

The **metadata review** step of the ingestion flow — the part that exists
because `kovan_literature::extract_metadata` is explicitly best-effort.

# Why this screen exists

`kovan-literature`'s own module docs say metadata is recovered "best-effort"
and that "a human reviewer fills gaps against the source". Until now nothing
in KOVAN gave that reviewer a place to stand: the CLI (`kovan lit import`)
prints the extracted record and writes it straight to disk. Wrong metadata
then flows into the generated BibTeX and from there into a citation, which
the workspace's `RESEARCH_INTEGRITY_AND_PROVENANCE.md` treats as a real
integrity problem rather than a cosmetic one.

A real, observed failure (2026-08-05, a 1977 Argonne benchmark-problem
report) motivated the design: extraction produced the correct title
(`ANL-7416 Supplement 2`) but `year: 2004` — a digitisation date from the
scan, not the publication year — an empty author list (the real corporate
author is "Argonne Code Center"), and therefore the slug `2004anl7416`.

# What it does

Presents every citation-critical field as an editable line, flags the fields
that are *typically* wrong ([`ReviewState::advisories`]), and re-derives the
slug/id from the **corrected** values before anything is written — so a
corrected year really does produce `argonnecodecenter1977anl7416`, not a slug
frozen at extraction time.

Nothing here mutates the extracted document in place: the pristine record
from `kovan-literature` is kept in [`ReviewState::extracted`] so the UI can
show what was changed, and a corrected copy is built on demand by
[`ReviewState::corrected_document`].

```rust
pub(in ::tui::ingest) mod review { /* ... */ }
```

### Types

#### Enum `ReviewField`

One editable row of the review form.

[`ReviewField::DocType`] is the only non-text row: it cycles through
[`DocumentType`] with Left/Right instead of accepting typed characters,
because the set of document types is closed (and picking from it cannot be
misspelled).

```rust
pub enum ReviewField {
    Title,
    Authors,
    Year,
    DocType,
    Institution,
    MarkdownOut,
    JsonOut,
    BibtexOut,
}
```

##### Variants

###### `Title`

Full document title (BibTeX `title`).

###### `Authors`

Author list, typed as `Family, Given; Family, Given` — see
[`parse_authors`].

###### `Year`

Publication year (BibTeX `year`), or empty for "unknown".

###### `DocType`

Document type; drives the BibTeX entry type (`@techreport`, `@article`…).

###### `Institution`

Issuing institution / awarding school (BibTeX `institution`/`school`).

###### `MarkdownOut`

Where to write the generated Markdown body (empty = don't write).

###### `JsonOut`

Where to write the canonical `KovanDocument` JSON (empty = don't write).

###### `BibtexOut`

Where to write the generated BibTeX entry (empty = don't write).

##### Implementations

###### Methods

- ```rust
  pub fn label(self: Self) -> &'static str { /* ... */ }
  ```
  Human-readable row label, as rendered in the form.

- ```rust
  pub fn step(self: Self, delta: i32) -> Self { /* ... */ }
  ```
  Move `delta` rows down (negative = up), wrapping at both ends.

- ```rust
  pub fn is_text(self: Self) -> bool { /* ... */ }
  ```
  Whether this row accepts typed text (everything except

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> ReviewField { /* ... */ }
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
    fn eq(self: &Self, other: &ReviewField) -> bool { /* ... */ }
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
#### Struct `ReviewState`

The editable review form for one extracted document.

Holds both the pristine extraction ([`ReviewState::extracted`]) and the
user's edits, so the UI can show which fields a human changed and the saved
record can be rebuilt from the corrected values.

```rust
pub struct ReviewState {
    pub source_pdf: std::path::PathBuf,
    pub extracted: kovan_common::KovanDocument,
    pub elapsed: std::time::Duration,
    pub title: crate::tui::text_input::TextInput,
    pub authors: crate::tui::text_input::TextInput,
    pub year: crate::tui::text_input::TextInput,
    pub document_type: kovan_common::DocumentType,
    pub institution: crate::tui::text_input::TextInput,
    pub markdown_out: crate::tui::text_input::TextInput,
    pub json_out: crate::tui::text_input::TextInput,
    pub bibtex_out: crate::tui::text_input::TextInput,
    pub outputs_pinned: bool,
    pub field: ReviewField,
    pub save_report: Vec<String>,
    pub preview_scroll: u16,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `source_pdf` | `std::path::PathBuf` | The PDF this document was extracted from. |
| `extracted` | `kovan_common::KovanDocument` | The untouched record returned by `kovan_literature::extract_metadata`.<br>Never mutated — it is the "what the extractor said" reference. |
| `elapsed` | `std::time::Duration` | Wall-clock time the extraction took, for display. |
| `title` | `crate::tui::text_input::TextInput` | Editable title. |
| `authors` | `crate::tui::text_input::TextInput` | Editable author list, in `Family, Given; …` form. |
| `year` | `crate::tui::text_input::TextInput` | Editable year, as typed (empty = unknown). |
| `document_type` | `kovan_common::DocumentType` | Selected document type (cycled, not typed). |
| `institution` | `crate::tui::text_input::TextInput` | Editable institution (empty = none). |
| `markdown_out` | `crate::tui::text_input::TextInput` | Output path for the generated Markdown body (empty = skip). |
| `json_out` | `crate::tui::text_input::TextInput` | Output path for the `KovanDocument` JSON (empty = skip). |
| `bibtex_out` | `crate::tui::text_input::TextInput` | Output path for the generated BibTeX entry (empty = skip). |
| `outputs_pinned` | `bool` | `true` once the user has hand-edited any output path, after which the<br>slug-derived defaults stop overwriting them. |
| `field` | `ReviewField` | Which row has focus. |
| `save_report` | `Vec<String>` | Lines written by the last [`ReviewState::save`] call (successes and<br>per-file errors); empty before the first save. |
| `preview_scroll` | `u16` | Vertical scroll offset of the derived-record pane. |

##### Implementations

###### Methods

- ```rust
  pub fn new(source_pdf: PathBuf, extracted: KovanDocument, elapsed: Duration) -> Self { /* ... */ }
  ```
  Build the form from a freshly extracted document.

- ```rust
  pub fn refresh_output_defaults(self: &mut Self) { /* ... */ }
  ```
  Recompute the three output paths from the *current* slug, unless the user

- ```rust
  pub fn visibility(self: &Self) -> Visibility { /* ... */ }
  ```
  Visibility of the document being reviewed — inherited from the extraction

- ```rust
  pub fn current_slug(self: &Self) -> String { /* ... */ }
  ```
  The slug the corrected record would carry, or the extracted slug when the

- ```rust
  pub fn focused_input_mut(self: &mut Self) -> Option<&mut TextInput> { /* ... */ }
  ```
  Mutable access to the focused text field, or `None` when the focused row

- ```rust
  pub fn field_value(self: &Self, field: ReviewField) -> String { /* ... */ }
  ```
  Current text of `field`, for rendering. The cycled type row renders its

- ```rust
  pub fn is_edited(self: &Self, field: ReviewField) -> bool { /* ... */ }
  ```
  Whether `field` now differs from what the extractor produced — rendered

- ```rust
  pub fn corrected_document(self: &Self) -> Result<KovanDocument, Vec<String>> { /* ... */ }
  ```
  Build the corrected [`KovanDocument`].

- ```rust
  pub fn advisories(self: &Self) -> Vec<String> { /* ... */ }
  ```
  Advisory notes about fields that extraction commonly gets wrong.

- ```rust
  pub fn save(self: &mut Self) -> bool { /* ... */ }
  ```
  Write the corrected record to whichever of the three output paths are

- ```rust
  pub(in ::tui::ingest::review) fn write_file(self: &mut Self, path: &str, contents: &str, kind: &str) -> bool { /* ... */ }
  ```
  Write one artifact, recording success or the exact error. Returns whether

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
### Functions

#### Function `step_document_type`

Cycle a [`DocumentType`] by `delta` positions, wrapping at both ends.

```rust
pub fn step_document_type(current: kovan_common::DocumentType, delta: i32) -> kovan_common::DocumentType { /* ... */ }
```

### Constants and Statics

#### Constant `DEFAULT_ARCHIVE_ROOT`

Default literature-archive root used to derive output paths, matching the
Literature tab's default (`kovan-tui` is normally launched from the
repository root). The generated sub-directories under it follow
`docs/kovan.md` § "Storage Layout" via
[`kovan_literature::storage::generated_dir_for`].

```rust
pub const DEFAULT_ARCHIVE_ROOT: &str = "crates/kovan-literature";
```

#### Constant `FIELDS`

Field order on screen, and the order Up/Down cycles through.

```rust
pub(in ::tui::ingest::review) const FIELDS: [ReviewField; 8] = _;
```

#### Constant `DOCUMENT_TYPES`

Every [`DocumentType`], in the order Left/Right cycles them.

```rust
pub(in ::tui::ingest::review) const DOCUMENT_TYPES: [kovan_common::DocumentType; 7] = _;
```

### Types

#### Enum `PickerField`

Which picker field the keyboard types into while editing.

```rust
pub enum PickerField {
    Root,
    Filter,
}
```

##### Variants

###### `Root`

The directory that is walked for PDFs.

###### `Filter`

A case-insensitive substring the PDF path must contain.

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
    fn clone(self: &Self) -> PickerField { /* ... */ }
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
    fn eq(self: &Self, other: &PickerField) -> bool { /* ... */ }
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
#### Struct `RunningJob`

A running extraction: the worker thread's handle to its result plus what the
UI needs to show that work is happening.

```rust
pub struct RunningJob {
    pub pdf: std::path::PathBuf,
    pub bytes: u64,
    pub started: std::time::Instant,
    pub frame: usize,
    pub(in ::tui::ingest) receiver: std::sync::mpsc::Receiver<Result<kovan_common::KovanDocument, String>>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pdf` | `std::path::PathBuf` | The PDF being extracted. |
| `bytes` | `u64` | Size of the source PDF in bytes (0 if it could not be stat'd) — shown so<br>a long wait on a large scan is understandable rather than alarming. |
| `started` | `std::time::Instant` | When the worker was spawned; drives the elapsed-time display. |
| `frame` | `usize` | Spinner frame counter, advanced once per [`IngestState::tick`]. |
| `receiver` | `std::sync::mpsc::Receiver<Result<kovan_common::KovanDocument, String>>` | One-shot result channel from the worker thread. |

##### Implementations

###### Methods

- ```rust
  pub fn spinner(self: &Self) -> char { /* ... */ }
  ```
  The current spinner character.

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
#### Struct `FailureReport`

A failed extraction, kept so the message stays on screen until dismissed.

```rust
pub struct FailureReport {
    pub pdf: std::path::PathBuf,
    pub message: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `pdf` | `std::path::PathBuf` | The PDF that failed. |
| `message` | `String` | What went wrong, as reported by `kovan-literature` (or by the worker<br>wrapper for a panic / dead thread). |

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
#### Enum `IngestPhase`

**Attributes:**

- `Other("#[allow(clippy::large_enum_variant)]")`

The tab's state machine. Enum dispatch, no trait objects — every phase is a
known variant and every `match` over it is exhaustive.

The `Review` variant is much larger than the others (it carries the whole
extracted [`KovanDocument`]). Clippy's usual remedy — boxing the large field
— is not available here: the workspace forbids `Box<T>` (root `CLAUDE.md`,
"Rust design rules"), and it would buy nothing, since exactly one
`IngestPhase` exists per running program.

```rust
pub enum IngestPhase {
    Picking,
    Running(RunningJob),
    Review(ReviewState),
    Failed(FailureReport),
}
```

##### Variants

###### `Picking`

Choosing a PDF to import.

###### `Running`

Extraction is running on a worker thread.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `RunningJob` |  |

###### `Review`

Extraction finished; the metadata is under human review.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `ReviewState` |  |

###### `Failed`

Extraction failed; showing why.

Fields:

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `FailureReport` |  |

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
#### Struct `IngestState`

State for the Ingest tab.

```rust
pub struct IngestState {
    pub root: super::text_input::TextInput,
    pub filter: super::text_input::TextInput,
    pub candidates: Vec<std::path::PathBuf>,
    pub list_state: ratatui::widgets::ListState,
    pub status: String,
    pub phase: IngestPhase,
    pub picker_field: PickerField,
    pub(in ::tui::ingest) edit_backup: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `root` | `super::text_input::TextInput` | Directory walked for candidate PDFs. |
| `filter` | `super::text_input::TextInput` | Case-insensitive substring filter over the discovered paths. |
| `candidates` | `Vec<std::path::PathBuf>` | PDFs found by the last scan, after filtering. |
| `list_state` | `ratatui::widgets::ListState` | Selection into [`IngestState::candidates`]. |
| `status` | `String` | One-line status message for the header. |
| `phase` | `IngestPhase` | Current phase. |
| `picker_field` | `PickerField` | Which picker field `e`/`f` edits. |
| `edit_backup` | `String` | Value of the focused field when the current edit began, restored on Esc. |

##### Implementations

###### Methods

- ```rust
  pub fn is_busy(self: &Self) -> bool { /* ... */ }
  ```
  `true` while an extraction is running — the draw loop polls faster then,

- ```rust
  pub fn blocks_quit(self: &Self) -> bool { /* ... */ }
  ```
  `true` when a global `q`/`Esc` would throw away work in progress (a

- ```rust
  pub fn help_line(self: &Self) -> &'static str { /* ... */ }
  ```
  The key-binding help line for the current phase, shown in the app's

- ```rust
  pub fn note_blocked_quit(self: &mut Self) { /* ... */ }
  ```
  Message shown when a quit was blocked by [`IngestState::blocks_quit`].

- ```rust
  pub fn run_scan(self: &mut Self) { /* ... */ }
  ```
  Re-run PDF discovery under the current root and filter.

- ```rust
  pub fn selected_pdf(self: &Self) -> Option<PathBuf> { /* ... */ }
  ```
  The currently selected candidate PDF, if any.

- ```rust
  pub fn start_extraction(self: &mut Self, pdf: PathBuf) { /* ... */ }
  ```
  Start extracting `pdf` on a worker thread and switch to the Running

- ```rust
  pub fn ingest_path(self: &mut Self, pdf: PathBuf) { /* ... */ }
  ```
  Hand-off entry point used by the Literature tab's `i` key: point the

- ```rust
  pub fn tick(self: &mut Self) -> bool { /* ... */ }
  ```
  Advance animation and collect a finished worker result.

- ```rust
  pub(in ::tui::ingest) fn focused_input_mut(self: &mut Self) -> Option<&mut TextInput> { /* ... */ }
  ```
  The text field the keyboard types into, given the current phase and

- ```rust
  pub(in ::tui::ingest) fn begin_edit(self: &mut Self, editing: &mut bool) { /* ... */ }
  ```
  Begin editing the focused field, remembering its value so `Esc` can put

- ```rust
  pub(in ::tui::ingest) fn commit_edit(self: &mut Self, editing: &mut bool) { /* ... */ }
  ```
  Finish an edit with `Enter`: rescan in the picker, or re-derive the

- ```rust
  pub(in ::tui::ingest) fn abandon(self: &mut Self) { /* ... */ }
  ```
  Abandon whatever is in flight and return to the picker.

- ```rust
  pub(in ::tui::ingest) fn select_next(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub(in ::tui::ingest) fn select_prev(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub fn handle_key(self: &mut Self, key: KeyEvent, editing: &mut bool) { /* ... */ }
  ```
  Handle one key event. `editing` is the shared edit-mode flag owned by

- ```rust
  pub(in ::tui::ingest) fn handle_picking_key(self: &mut Self, key: KeyEvent, editing: &mut bool) { /* ... */ }
  ```
  Keys for the PDF picker: edit the root/filter, scan, move the selection,

- ```rust
  pub(in ::tui::ingest) fn handle_review_key(self: &mut Self, key: KeyEvent, editing: &mut bool) { /* ... */ }
  ```
  Keys for the metadata review form: move between rows, edit a row, cycle

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn default() -> Self { /* ... */ }
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

- **Read**
- **ReadPrimitive**
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
### Functions

#### Function `spawn_extraction`

Spawn the worker thread that runs `kovan_literature::extract_metadata`.

The library call is wrapped in [`std::panic::catch_unwind`] because PDF
parsing runs over untrusted third-party bytes: a panic there must surface as
a `Failed` phase in the UI, not as a dead process with the terminal left in
raw mode. The thread is detached — abandoning a job simply drops the
receiving end, and the worker's `send` then fails harmlessly.

```rust
pub(in ::tui::ingest) fn spawn_extraction(pdf: std::path::PathBuf, bytes: u64) -> RunningJob { /* ... */ }
```

### Constants and Statics

#### Constant `SPINNER`

Frames of the liveness spinner shown while extraction runs.

```rust
pub(in ::tui::ingest) const SPINNER: [char; 4] = _;
```

### Re-exports

#### Re-export `draw`

```rust
pub use draw::draw;
```

#### Re-export `ReviewField`

```rust
pub use review::ReviewField;
```

#### Re-export `ReviewState`

```rust
pub use review::ReviewState;
```

## Module `literature`

Literature tab — a human-facing view over `kovan_literature`.

Type a literature-crate root (defaults to `crates/kovan-literature`,
matching this workspace's layout when `kovan-tui` is run from the
repository root — `docs/kovan.md` § "Storage Layout"), press Enter to list
PDFs / Markdown / BibTeX under it, then press Enter again on a selected
entry to preview it:

- `.pdf` — best-effort [`kovan_literature::extract_metadata`] fields
  (title, authors, year, DOI, …) plus the start of the generated Markdown
  body.
- `.md` / `.markdown` — the heading outline
  ([`kovan_literature::markdown_outline`]) followed by the raw text.
- `.bib` — the raw BibTeX text as written
  ([`kovan_literature::to_bibtex`] generates these files; this tab only
  reads them back).

Like every other tab, this is a **read-only viewer** — it never writes
into the `open/`/`proprietary/`/`generated/` storage tree it browses.

```rust
pub(in ::tui) mod literature { /* ... */ }
```

### Types

#### Enum `LitKind`

Which entry kinds are listed. Unlike [`super::browser::KindFilter`] this is
scoped to the three file types the literature pipeline cares about, plus a
`.bib` kind that [`kovan_discovery::FileKind`] doesn't model (bibliography
files aren't one of that enum's general-purpose categories).

```rust
pub enum LitKind {
    All,
    Pdf,
    Markdown,
    Bib,
}
```

##### Variants

###### `All`

###### `Pdf`

###### `Markdown`

###### `Bib`

##### Implementations

###### Methods

- ```rust
  pub(in ::tui::literature) fn label(self: Self) -> &'static str { /* ... */ }
  ```

- ```rust
  pub(in ::tui::literature) fn step(self: Self, delta: i32) -> Self { /* ... */ }
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
    fn clone(self: &Self) -> LitKind { /* ... */ }
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
    fn eq(self: &Self, other: &LitKind) -> bool { /* ... */ }
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
#### Struct `LiteratureState`

State for the Literature tab.

```rust
pub struct LiteratureState {
    pub root: super::text_input::TextInput,
    pub kind: LitKind,
    pub entries: Vec<std::path::PathBuf>,
    pub list_state: ratatui::widgets::ListState,
    pub status: String,
    pub preview: String,
    pub preview_scroll: u16,
    pub ingest_request: Option<std::path::PathBuf>,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `root` | `super::text_input::TextInput` |  |
| `kind` | `LitKind` |  |
| `entries` | `Vec<std::path::PathBuf>` |  |
| `list_state` | `ratatui::widgets::ListState` |  |
| `status` | `String` |  |
| `preview` | `String` |  |
| `preview_scroll` | `u16` |  |
| `ingest_request` | `Option<std::path::PathBuf>` | Set when the user presses `i` on a selected PDF: a request for<br>[`super::App`] to hand this file to the Ingest tab. Drained by<br>[`LiteratureState::take_ingest_request`] — this tab never imports<br>anything itself, keeping it a read-only viewer. |

##### Implementations

###### Methods

- ```rust
  pub(in ::tui::literature) fn run_scan(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub(in ::tui::literature) fn select_next(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub(in ::tui::literature) fn select_prev(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub(in ::tui::literature) fn preview_selected(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub(in ::tui::literature) fn request_ingest(self: &mut Self) { /* ... */ }
  ```
  Ask for the selected entry to be imported on the Ingest tab.

- ```rust
  pub fn take_ingest_request(self: &mut Self) -> Option<PathBuf> { /* ... */ }
  ```
  Take a pending ingest request, if any, leaving `None` behind. Called by

- ```rust
  pub fn handle_key(self: &mut Self, key: KeyEvent, editing: &mut bool) { /* ... */ }
  ```
  Handle one key event. `editing` is the shared edit-mode flag owned by

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn default() -> Self { /* ... */ }
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

- **Read**
- **ReadPrimitive**
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
### Functions

#### Function `format_document_preview`

```rust
pub(in ::tui::literature) fn format_document_preview(doc: &kovan_common::KovanDocument) -> String { /* ... */ }
```

#### Function `format_markdown_preview`

```rust
pub(in ::tui::literature) fn format_markdown_preview(headings: &[kovan_literature::Heading], text: &str) -> String { /* ... */ }
```

#### Function `draw`

Render the Literature tab into `area`.

```rust
pub fn draw(frame: &mut ratatui::Frame<''_>, area: ratatui::layout::Rect, state: &mut LiteratureState, editing: bool) { /* ... */ }
```

### Constants and Statics

#### Constant `KINDS`

```rust
pub(in ::tui::literature) const KINDS: [LitKind; 4] = _;
```

## Module `methods`

Numerical-Method Catalogue tab — a human-facing view over `kovan_codegen`.

Cycle the method family (root finders / linear solvers / nonlinear solvers
/ ODE solvers / PDE schemes) with Left/Right, the method within a family
with Up/Down, and press Enter to generate and preview its source
([`kovan_codegen::generate`]). Entries that are catalogued but not yet
backed by a template show the same [`kovan_codegen::CodegenError`] message
the `kovan` CLI's `methods` subcommand reports.

```rust
pub(in ::tui) mod methods { /* ... */ }
```

### Types

#### Enum `Family`

A method family — one screen "column" of the catalogue. Mirrors
`docs/kovan.md` § "Numerical Methods" (Root Finding / Linear Solvers /
Nonlinear Solvers / ODE Solvers / PDE Infrastructure).

```rust
pub enum Family {
    Root,
    Linear,
    Nonlinear,
    Ode,
    Pde,
}
```

##### Variants

###### `Root`

###### `Linear`

###### `Nonlinear`

###### `Ode`

###### `Pde`

##### Implementations

###### Methods

- ```rust
  pub(in ::tui::methods) fn label(self: Self) -> &'static str { /* ... */ }
  ```

- ```rust
  pub(in ::tui::methods) fn step(self: Self, delta: i32) -> Self { /* ... */ }
  ```

- ```rust
  pub(in ::tui::methods) fn methods(self: Self) -> Vec<Method> { /* ... */ }
  ```
  Every catalogued method in this family, in the same order the `kovan`

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> Family { /* ... */ }
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
    fn eq(self: &Self, other: &Family) -> bool { /* ... */ }
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
#### Struct `MethodsState`

State for the Numerical-Method Catalogue tab.

```rust
pub struct MethodsState {
    pub family: Family,
    pub list_state: ratatui::widgets::ListState,
    pub preview: String,
    pub preview_scroll: u16,
    pub status: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `family` | `Family` |  |
| `list_state` | `ratatui::widgets::ListState` |  |
| `preview` | `String` |  |
| `preview_scroll` | `u16` |  |
| `status` | `String` |  |

##### Implementations

###### Methods

- ```rust
  pub(in ::tui::methods) fn select_next(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub(in ::tui::methods) fn select_prev(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub(in ::tui::methods) fn generate_selected(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub fn handle_key(self: &mut Self, key: KeyEvent) { /* ... */ }
  ```
  Handle one key event. This tab has no text-editing field, so it has no

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn default() -> Self { /* ... */ }
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

- **Read**
- **ReadPrimitive**
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
### Functions

#### Function `method_label`

A short display label for a [`Method`] — its variant name via [`Debug`].

```rust
pub(in ::tui::methods) fn method_label(m: kovan_codegen::Method) -> String { /* ... */ }
```

#### Function `draw`

Render the Methods tab into `area`.

```rust
pub fn draw(frame: &mut ratatui::Frame<''_>, area: ratatui::layout::Rect, state: &mut MethodsState) { /* ... */ }
```

### Constants and Statics

#### Constant `FAMILIES`

```rust
pub(in ::tui::methods) const FAMILIES: [Family; 5] = _;
```

## Module `overview`

Overview tab — the original static module map, kept as the landing screen.

No state, no interaction beyond tab-switching: this is the "what is KOVAN"
screen a first-time user lands on.

```rust
pub(in ::tui) mod overview { /* ... */ }
```

### Functions

#### Function `draw`

Render the Overview tab into `area`.

```rust
pub fn draw(frame: &mut ratatui::Frame<''_>, area: ratatui::layout::Rect) { /* ... */ }
```

## Module `symbols`

Symbol Catalogue tab — a human-facing view over `kovan_semantics`.

Point at a repository root, cycle the [`LanguageAdapter`], and press Enter
to run the ripgrep-first catalogue
([`kovan_semantics::catalogue_symbols_detailed`]). Press `m` to flip
between the raw symbol list and the generated `symbols.md` Markdown
artifact ([`kovan_semantics::symbols_markdown`]) — the exact text KOVAN
would write to disk, previewed here instead.

```rust
pub(in ::tui) mod symbols { /* ... */ }
```

### Types

#### Struct `SymbolsState`

State for the Symbol Catalogue tab.

```rust
pub struct SymbolsState {
    pub root: super::text_input::TextInput,
    pub adapter: kovan_semantics::LanguageAdapter,
    pub symbols: Vec<kovan_common::KovanSymbol>,
    pub list_state: ratatui::widgets::ListState,
    pub status: String,
    pub markdown_view: bool,
    pub markdown_scroll: u16,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `root` | `super::text_input::TextInput` |  |
| `adapter` | `kovan_semantics::LanguageAdapter` |  |
| `symbols` | `Vec<kovan_common::KovanSymbol>` |  |
| `list_state` | `ratatui::widgets::ListState` |  |
| `status` | `String` |  |
| `markdown_view` | `bool` |  |
| `markdown_scroll` | `u16` |  |

##### Implementations

###### Methods

- ```rust
  pub(in ::tui::symbols) fn run_catalogue(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub(in ::tui::symbols) fn select_next(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub(in ::tui::symbols) fn select_prev(self: &mut Self) { /* ... */ }
  ```

- ```rust
  pub fn handle_key(self: &mut Self, key: KeyEvent, editing: &mut bool) { /* ... */ }
  ```
  Handle one key event. `editing` is the shared edit-mode flag owned by

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn default() -> Self { /* ... */ }
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

- **Read**
- **ReadPrimitive**
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
### Functions

#### Function `adapter_label`

```rust
pub(in ::tui::symbols) fn adapter_label(a: kovan_semantics::LanguageAdapter) -> &'static str { /* ... */ }
```

#### Function `adapter_step`

```rust
pub(in ::tui::symbols) fn adapter_step(a: kovan_semantics::LanguageAdapter, delta: i32) -> kovan_semantics::LanguageAdapter { /* ... */ }
```

#### Function `draw`

Render the Symbols tab into `area`.

```rust
pub fn draw(frame: &mut ratatui::Frame<''_>, area: ratatui::layout::Rect, state: &mut SymbolsState, editing: bool) { /* ... */ }
```

### Constants and Statics

#### Constant `ADAPTERS`

```rust
pub(in ::tui::symbols) const ADAPTERS: [kovan_semantics::LanguageAdapter; 4] = _;
```

## Module `text_input`

A minimal single-line text-editing buffer shared by every tab that needs a
path or pattern field (repository root, in the Browser/Symbols/Literature
tabs).

Deliberately tiny: append-at-end / backspace-at-end / set / clear. There is
no cursor-in-the-middle editing, no selection, no clipboard — KOVAN's TUI is
a viewer/navigator over deterministic sibling-crate output, not a text
editor, so this is scoped to "type a path, press Enter."

```rust
pub(in ::tui) mod text_input { /* ... */ }
```

### Types

#### Struct `TextInput`

A single line of user-typed text (a filesystem path, in every current
caller). Insertion is always at the end; there is no cursor position to
track because nothing in this crate needs to edit the middle of a path.

```rust
pub struct TextInput {
    pub(in ::tui::text_input) value: String,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `value` | `String` |  |

##### Implementations

###### Methods

- ```rust
  pub fn new</* synthetic */ impl Into<String>: Into<String>>(initial: impl Into<String>) -> Self { /* ... */ }
  ```
  Start with `initial` already in the buffer (e.g. a sensible default

- ```rust
  pub fn value(self: &Self) -> &str { /* ... */ }
  ```
  The current contents.

- ```rust
  pub fn push_char(self: &mut Self, c: char) { /* ... */ }
  ```
  Append one character at the end.

- ```rust
  pub fn backspace(self: &mut Self) { /* ... */ }
  ```
  Remove the last character, if any. No-op on an empty buffer.

- ```rust
  pub fn clear(self: &mut Self) { /* ... */ }
  ```
  Empty the buffer.

- ```rust
  pub fn set</* synthetic */ impl Into<String>: Into<String>>(self: &mut Self, value: impl Into<String>) { /* ... */ }
  ```
  Replace the buffer's contents wholesale. See [`clear`](Self::clear) for

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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
    fn clone(self: &Self) -> TextInput { /* ... */ }
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
    fn default() -> TextInput { /* ... */ }
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
    fn eq(self: &Self, other: &TextInput) -> bool { /* ... */ }
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
### Types

#### Enum `Tab`

The six human-facing screens.

```rust
pub enum Tab {
    Overview,
    Browser,
    Symbols,
    Methods,
    Literature,
    Ingest,
}
```

##### Variants

###### `Overview`

###### `Browser`

###### `Symbols`

###### `Methods`

###### `Literature`

###### `Ingest`

Interactive literature ingestion — the only screen that writes files.

##### Implementations

###### Methods

- ```rust
  pub(in ::tui) fn title(self: Self) -> &'static str { /* ... */ }
  ```

- ```rust
  pub(in ::tui) fn next(self: Self) -> Self { /* ... */ }
  ```

- ```rust
  pub(in ::tui) fn prev(self: Self) -> Self { /* ... */ }
  ```

- ```rust
  pub(in ::tui) fn from_digit(c: char) -> Option<Self> { /* ... */ }
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
#### Struct `App`

Top-level application state: the active tab, whether a text field is being
edited, and one state struct per tab (see the module docs on why these are
owned by value with no lock).

```rust
pub struct App {
    pub tab: Tab,
    pub editing: bool,
    pub should_quit: bool,
    pub(in ::tui) browser: browser::BrowserState,
    pub(in ::tui) symbols: symbols::SymbolsState,
    pub(in ::tui) methods: methods::MethodsState,
    pub(in ::tui) literature: literature::LiteratureState,
    pub(in ::tui) ingest: ingest::IngestState,
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| `tab` | `Tab` |  |
| `editing` | `bool` | `true` while the active tab's text-input field (repository/literature<br>root) is capturing keystrokes instead of navigation keys. |
| `should_quit` | `bool` |  |
| `browser` | `browser::BrowserState` |  |
| `symbols` | `symbols::SymbolsState` |  |
| `methods` | `methods::MethodsState` |  |
| `literature` | `literature::LiteratureState` |  |
| `ingest` | `ingest::IngestState` |  |

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

- ```rust
  pub(in ::tui) fn poll_interval(self: &Self) -> Duration { /* ... */ }
  ```
  How long the draw loop should wait for a key before looping again —

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
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

- **Read**
- **ReadPrimitive**
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
### Functions

#### Function `run`

Set up the terminal, run the draw/input loop, and restore on exit (or on a
draw/read error — `ratatui::restore()` always runs, and `ratatui::init()`
installs a panic hook that restores first, so neither a panic nor an I/O
error can leave the user's terminal in raw mode).

```rust
pub fn run() -> std::io::Result<()> { /* ... */ }
```

#### Function `draw_loop`

Draw, wait briefly for input, tick background work, repeat.

The wait is `event::poll` rather than a blocking `event::read` so a running
PDF extraction can keep its elapsed-time display current and deliver its
result without the user touching the keyboard.

```rust
pub(in ::tui) fn draw_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> std::io::Result<()> { /* ... */ }
```

#### Function `draw`

```rust
pub(in ::tui) fn draw(frame: &mut ratatui::Frame<''_>, app: &mut App) { /* ... */ }
```

### Constants and Statics

#### Constant `TABS`

```rust
pub(in ::tui) const TABS: [Tab; 6] = _;
```

## Functions

### Function `main`

**Attributes:**

- `Other("#[attr = CfgTrace([Not(NameValue { name: \"target_os\", value: Some(\"android\"), span: crates/kovan-tui/src/main.rs:35:11: 35:32 (#0) }, crates/kovan-tui/src/main.rs:35:10: 35:33 (#0))])]")`

```rust
pub(crate) fn main() -> std::io::Result<()> { /* ... */ }
```

