# kovan-common — decisions log

Written for human review after the "solidify the shared-types foundation" pass
(2026-07-15). Every non-trivial choice below needs a human skim; none of it is
a load-bearing physics/engineering decision, so risk is low, but several
choices affect the API every other `kovan-*` crate will code against.

## 1. The `// TODO(kovan)` marker

`grep -rn 'TODO(kovan)' crates/kovan-common/src` finds exactly one hit, and it
is **not** an unimplemented function — it's a line in the module-level doc
comment (`//! most helper logic is a `// TODO(kovan)` stub.`) that describes
the *whole KOVAN workspace's* placeholder stage, copy-pasted into this crate's
doc comment from the same boilerplate used in `kovan-literature`,
`kovan-semantics`, and `kovan-codegen`. Those other three crates do have real
stub functions returning `Unimplemented`/similar; **kovan-common does not** —
every function that existed before this pass (`KovanDocument::new`) was fully
implemented, not a stub.

**Resolution:** treated the marker as stale/inaccurate for this crate and
replaced the doc-comment paragraph with a "## Maturity" section that states
plainly that kovan-common is a complete, fully-implemented data crate, and
that the other kovan-* crates' own `// TODO(kovan)` markers are separate and
still open. I did not invent a new stub function to "resolve" against, since
manufacturing an unimplemented placeholder in an otherwise-complete crate
would be the wrong kind of change. If this reading is wrong and there was a
specific intended stub, it isn't in this crate's source as it stands today.

## 2. `#[serde(default)]` on every `Vec<T>` field of `KovanDocument`

**Problem found (not assumed — verified with a throwaway test, since removed):**
without `#[serde(default)]`, `serde_json`/`toml` hard-error
(`missing field 'tags'`) when deserializing a document JSON/TOML blob that
predates a field being added to the struct. `Option<T>` fields don't have
this problem — serde already treats a missing key as `None` for `Option`
fields with no attribute needed, confirmed by test.

Since `kovan-literature`'s stated storage model is `KovanDocument` persisted
to disk (open/proprietary/generated trees) and read back across the life of
the project, and the struct itself has already grown once (per the spec:
`related_symbols`/`related_repositories`/`related_benchmarks`), a schema
addition happening again is the expected case, not a hypothetical one.

**Decision:** added `#[serde(default)]` to all six `Vec<T>` fields
(`authors`, `keywords`, `tags`, `related_symbols`, `related_repositories`,
`related_benchmarks`). Left the identity/classification fields (`id`, `slug`,
`visibility`, `document_type`, `title`) and the two plain-`String` body
fields (`abstract_text`, `markdown_body`) *without* a default — those staying
required is a feature: a document silently missing its title/id because of a
day-one bug should still fail loudly. New test
`document_json_missing_newer_fields_still_deserialises` locks in the intended
contract (documents with the six Vec fields entirely absent still parse) so a
future edit can't silently regress it.

Applied the same defensive framing to the other struct comments (documented
what `None`/empty means for every optional field) since it was previously
implicit.

## 3. `Hash` added to `Visibility` and `DocumentType`

Both are small, closed, `Copy` enums. No current code groups documents by
type/visibility in a `HashMap`/`HashSet`, but this is the single most likely
thing a CLI/TUI consumer will want to do (e.g. "group the literature index by
`DocumentType`" for a listing view), and `derive(Hash)` on a C-like enum is
free (no behavioural risk, no API-breaking surface beyond adding a trait
impl). Did **not** add `Ord`/`PartialOrd` — no evidence yet that a defined
ordering (e.g. "Paper before Report") is meaningful, and getting that wrong
would be worse than not having it; easy to add later once a real consumer
needs it.

## 4. Did *not* add a `Display`/`FromStr` impl for the enums

`kovan-literature`'s BibTeX/Markdown generation will eventually need to
render `DocumentType`/`Visibility` as text, and `kovan-cli` will likely want
to parse `--type paper` from an argument. Left this out because the exact
string form (`"paper"` vs `"Paper"` vs a BibTeX-specific vocabulary like
`@article`/`@techreport`) is a decision that belongs to the consuming crate,
and guessing now risks a form that has to be reworked. **Flagging as an open
question for whoever picks up `kovan-literature`'s BibTeX renderer or
`kovan-cli`'s argument parsing** — come back to kovan-common if/when a
canonical string form is needed by more than one consumer (per the
"single source of truth, don't duplicate core types" rule, that canonical
string form belongs here once two crates need the same one).

## 5. Did *not* add a result/measurement type to `KovanValidationCase`

The spec only sketches `KovanValidationCase` as "ties an implementation to a
benchmark/correlation and its measured result" without giving the shape of
the result. A k-eigenvalue benchmark result (`k_eff ± σ`), a correlation
accuracy check (residual stats against a table), and a transient TH
comparison (time-series RMSE) don't share a schema, and no consumer exists
yet to reveal what's actually needed. Documented this explicitly in the
struct's doc comment rather than silently leaving it out, so it doesn't read
as an oversight. **Open question for the human:** should this eventually be
an enum (`ValidationResult::KEffPcm { .. } | ValidationResult::CorrelationFit { .. } | ...`)
per the workspace's "enum dispatch, not trait objects" rule? That's the
natural shape once there's a second kind of result to compare against the
first — recommend waiting for `kovan-semantics` or a downstream V&V consumer
to define the first two concrete cases before locking in the enum's variants.

## 6. `Author` doc comment: documented the organisational-author convention

The existing `examples/build_document.rs` (pre-existing, unmodified) sets
`Author { family: "ICSBEP", given: "", affiliation: Some("OECD/NEA") }` for
an ICSBEP evaluation "author." That convention (empty `given` for an
organisation) was implicit in the example but undocumented on the type
itself — a new consumer reading only the struct's rustdoc (per the workspace's
"navigable by rust-analyzer alone" rule) had no way to discover it. Documented
it on `Author` directly; did not change the type shape (no `Organisation`
variant/enum) since a plain-string convention already works and the spec's
canonical struct doesn't carve out that distinction.

## 7. `examples/build_document.rs` — left as-is

One already existed (not created by this pass) and already demonstrates
constructing a `KovanDocument` end-to-end (including the organisational-author
convention from #6), matching the deliverable's "if none exists" condition.
Considered extending it to also show `serde_json`/`toml` serialisation to
disk, but that would start encoding a storage-layout opinion
(`kovan-literature`'s `open/`/`proprietary/`/`generated/` tree) that belongs
to that crate, not to a types-only example here. Left it to
`kovan-literature` to add that example once its storage code is real.

## 8. TOML round-trip: no serde attribute needed beyond `#[serde(default)]`

Verified empirically (see item 2) that `toml 0.8`'s `Serializer` already
omits `None`-valued fields when writing, and skips `Some`/absent handling
correctly on read via serde's built-in `Option` behaviour — no
`#[serde(skip_serializing_if = "Option::is_none")]` needed anywhere. Added
TOML round-trip tests for every struct type (`KovanDocument`, `Author`,
`KovanSymbol`, `KovanRepository`, `KovanCorrelation`, `KovanBenchmark`,
`KovanValidationCase`) including populated-`Option`, empty-`Option`, and
fully-populated-document variants, to lock this behaviour in against future
`toml`/`serde` upgrades. Did not add a TOML round-trip test for the bare
`Visibility`/`DocumentType` enums standalone — TOML documents must be a table
at the root, so a bare enum can't round-trip through `toml::to_string` at the
top level; they're already exercised via every struct that embeds them.

## Beads / follow-up (not filed — `op-5v5` is JSONL-only, see task brief)

Would file, if the epic were writable:
- `kovan-literature`: decide the BibTeX-facing string form for
  `DocumentType`/`Visibility` (feeds back into kovan-common per decision #4
  once a second consumer needs the same string form).
- `kovan-semantics` or a future V&V consumer: define the first 1-2 concrete
  `KovanValidationCase` result shapes, then design the
  `ValidationResult` enum in kovan-common (decision #5).
- `kovan-codegen`: currently depends on `kovan-common` but doesn't import
  anything from it yet (`grep kovan_common crates/kovan-codegen/src` — no
  hits). The natural link is `KovanCorrelation` — a generated numerical
  method implementing a correlation should be traceable back to the
  correlation record. No action taken now; flagging so it isn't lost.

## v2 pass (2026-07-15) — shared-type additions landed

Follow-up pass that implemented the "needs from kovan-common" the three
downstream crates reported (see `docs/kovan-agent-decisions-for-review.md`
call #1). All additions are additive and every kovan crate stays green
(build/test/clippy `-D warnings`/`fmt`/`doc`/`cargo check
--target aarch64-linux-android`).

**Crate split.** The crate grew past the workspace's <1000-line file cap, so
`lib.rs` was split into a module dir (`document.rs`, `symbol.rs`,
`knowledge.rs`); `lib.rs` is now crate docs + `pub use` re-exports, so every
existing `kovan_common::TypeName` import still resolves.

**`Language` enum** (`symbol.rs`) — `Rust` / `Cpp` / `Python` / `Fortran`,
serde + `Display`/`as_str`. Common now owns language identity; `kovan-semantics`
keeps its `LanguageAdapter` (which also selects the language *server* and file
extensions) and converts via a new `From<LanguageAdapter> for Language`.

**`KovanSymbol` location/language** — added `file: String` (repo-relative),
`line: u32`, `language: Language`. `ExtractedSymbol::into_kovan_symbol` fills
them. These are required fields (no `#[serde(default)]`) — there is no
serialised-symbol-on-disk back-compat contract as there is for documents.
- **`ExtractedSymbol` was reduced, not retired.** The location-field
  duplication that motivated it is resolved (KovanSymbol now carries file/line/
  language), and `outputs::{symbols_markdown, repository_summary_markdown}` plus
  the cli/tui symbol views were all rewired onto the shared `KovanSymbol`.
  `ExtractedSymbol` remains as the scanners' **file-local** intermediate: a
  `KovanSymbol` additionally needs the `id`/`repository_id` that only the
  catalogue step (which knows the repo) supplies, and it carries a typed
  `SymbolKind` where `KovanSymbol.kind` is free text. So it is a distinct
  extraction-stage record, not a duplicate core type.
- **`KovanModule` — deferred.** `kovan-semantics` only models a module as a
  `SymbolKind::Module` symbol and no consumer needs a distinct struct; adding
  one now would be speculative (same YAGNI reasoning as the deferred
  `KovanValidationCase` result type). Revisit when a module-graph consumer lands.
- **Enum `kind` — deferred.** Kept `KovanSymbol.kind` as `String`: the
  rust-analyzer/clangd/Pyright/fortls vocabularies don't line up, and
  `kovan-semantics` owns the closed `SymbolKind` and stringifies via `as_str`.
  Adopt into common only once a consumer needs to match on it.

**`KovanDocument` additions** (`document.rs`, all forward-compatible —
`Vec` fields `#[serde(default)]`, `Option`s default to `None`): `volume`,
`pages`, `number` (journal locators, all `Option<String>` since they aren't
always numeric), `source_path` + `source_sha256` (`Option<String>`),
`page_count: Option<u32>`, `assets: Vec<String>`. The
`document_json_missing_newer_fields_still_deserialises` regression test was
extended to assert every one of these defaults when absent.
- **`KovanDocumentBuilder`** — ergonomic builder (owns the doc by value,
  chained `self` setters, infallible `build()`; no trait objects/lifetimes).
  `KovanDocument::builder(...)` starts it; `new(...)` kept for the bare path.
  `examples/build_document.rs` now uses it.

**`GeneratedArtifact`** (`knowledge.rs`) — provenance record
`{ method: String, source: String, correlation_id: Option<String>,
source_document_id: Option<String> }` for the Paper→Correlation→Implementation
link. **Wired** in `kovan-codegen::generate_artifact(method, correlation_id,
source_document_id)` (small helper over `generate`; `method` recorded as the
`Debug` label, deterministic).

**Downstream rewiring.** `kovan-literature::extract_metadata` now builds via
the builder and populates `source_path` + `page_count` (counted from
form-feed page breaks); `to_bibtex` emits `volume`/`number`/`pages` when set.
`source_sha256` is left `None` — a real SHA-256 would need a `sha2` dependency,
which was **not** added (no new deps this pass); flagged here as the one field
the literature pipeline can't fill offline without a dep decision.
`kovan-cli`/`kovan-tui` surface `page_count`/`source_path` and the journal
locators in their summaries/previews.

## What other kovan crates likely need from kovan-common next

- `kovan-literature` already pulls `Author, DocumentType, KovanBenchmark,
  KovanDocument, Visibility` — no new needs surfaced by this pass beyond
  decision #4 (string form for BibTeX rendering, when it's implemented).
- `kovan-semantics` already pulls `KovanRepository, KovanSymbol,
  KovanValidationCase` — decision #5 (the validation-result shape) is the
  main blocker for that crate doing anything beyond linking IDs together.
- `kovan-codegen` depends on `kovan-common` in `Cargo.toml` but has no `use`
  of it yet; `KovanCorrelation` is the type it will need once code generation
  is wired to a correlation record (see follow-up above).
- `kovan-cli`/`kovan-tui`: no direct type needs surfaced yet beyond whatever
  `kovan-literature`/`kovan-semantics` re-export.
