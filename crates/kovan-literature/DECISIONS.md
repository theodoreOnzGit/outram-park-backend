# kovan-literature — implementation decisions

Log for the literature pipeline work (Agent C "Literature Pipeline" + Agent D
"BibTeX Pipeline"). Scope: fleshing out the five `// TODO(kovan)` stubs into a
real, offline, deterministic, Android-first `PDF → Markdown → KovanDocument →
BibTeX → generated artifacts` pipeline. See `docs/kovan.md`, sections
"Literature Workflow", "Canonical Representation", "PDF Processing".

## PDF text-extraction crate: `pdf-extract` 0.12 (WIRED, not stubbed)

**Decision: use `pdf-extract` 0.12** for real, offline, pure-Rust text
extraction. It met every constraint, so PDF import is implemented, not stubbed.

Evaluation of the three candidates:

| Crate | Verdict |
|---|---|
| `pdf-extract` 0.12 | **Chosen.** Pure-Rust, does layout-aware text extraction out of the box. |
| `lopdf` 0.42 | Adopted *as well*, but only as the low-level object model (metadata + assets); it does not do text-with-layout extraction on its own. |
| `pdf` crate | Not needed once `pdf-extract` cleared the bar. |

Why `pdf-extract` passes the KOVAN gates:

- **Offline / local-first:** no network, no OCR, no cloud. Pure algorithmic text
  extraction from the PDF content streams.
- **Android-first:** `cargo check -p kovan-literature --target
  aarch64-linux-android --release` passes. The whole transitive tree is
  pure-Rust with **no C/native build deps** (verified: no `cc`/`cmake`/`bindgen`/
  `*-sys`/`pkg-config`/OpenSSL). `flate2` resolves to the pure-Rust
  `miniz_oxide` backend; `getrandom` uses the Android syscall path.
- **Licence:** `pdf-extract` and `lopdf` are **MIT**; the entire transitive
  closure is permissive (MIT / Apache-2.0 / BSD-3-Clause / Zlib / Unicode-3.0 /
  0BSD), all **GPLv3-compatible**. No copyleft conflicts.
- **Determinism:** extraction is a pure function of the input bytes.

Robustness note: `pdf-extract` can **panic** on some malformed/unsupported PDFs.
`extract_pdf_text` wraps the call in `std::panic::catch_unwind` and converts a
panic into `LiteratureError::Io` so a bad input never aborts the caller. (A
panic still prints via the default hook; that is cosmetic.)

## Assets: partial, honest implementation (`extract_assets`)

`extract_assets(pdf, out_dir)` extracts embedded raster images via `lopdf`, but
**only** those whose PDF filter is already a standalone file format:
`DCTDecode` → `.jpg`, `JPXDecode` → `.jp2` (their stream bytes *are* the encoded
file, so writing them is lossless and trivial). Images under other filters (raw
`FlateDecode` samples, `CCITTFax`, `JBIG2`, …) would need re-encoding to PNG with
the correct colour space / bit depth / decode array — that is a real image-codec
project, so those are **skipped and reported, not fabricated**. Output is written
in deterministic PDF object-id order, named `<pdf-stem>-imgNNN.<ext>`; `out_dir`
is created only if at least one image is written.

The signature gained an explicit `out_dir: &Path` parameter (the stub took only
the PDF). There were no external callers except the crate's own example, so this
was safe; it keeps the function testable and free of any CWD/`env!` assumption.

## Markdown generation (`text_to_markdown`, `pdf_to_markdown`)

Deterministic flat-text → Markdown transform:

1. Split on form-feed (`U+000C`) page breaks that `pdf-extract` emits.
2. Group non-blank line runs into paragraphs; join wrapped lines with a space;
   undo end-of-line hyphenation (`combus-\ntion` → `combustion`).
3. Promote **high-confidence** headings only: numbered section headers
   (`1 Introduction`, `2.1 Governing Equations`, level = `1 + dotted-depth`) and
   a fixed allow-list of section keywords (`Abstract`, `References`, …) → `##`.
   Everything else stays paragraph text. The converter **does not invent**
   headings the source lacks (tested).
4. Join pages with `PAGE_SEPARATOR` (`\n\n---\n\n`).

`split_markdown_by_page_limit(md, max_pages)` chunks the body on those page
boundaries so no generated document exceeds the `≤ 30`-page target
(`MAX_MARKDOWN_PAGES`). `markdown_outline` (pre-existing, kept) parses any
Markdown with `pulldown-cmark` into a heading outline.

## Metadata extraction heuristics (`extract_metadata`) + limits

Best-effort, in descending order of trust:

1. **PDF Info dictionary** (`/Title`, `/Author`, `/Keywords`, `/CreationDate`),
   read losslessly via `lopdf`. Text strings decoded as UTF-16BE when they carry
   a `FE FF` BOM, else PDFDocEncoding≈Latin-1. Most reliable; used when present.
2. **Text fallbacks** (only when the Info field is missing):
   - title → first "substantial" line (8–200 chars, not a URL/DOI/email line);
   - year → first plausible `1900–2099` integer in the opening 3000 chars
     (**noisy** — may catch a citation year; low-trust, documented as such);
   - DOI → first `10.<≥4 digits>/…` match anywhere (trailing punctuation
     trimmed). `10.5` alone is rejected (too few registrant digits).
3. **Storage-path hints:** `Visibility` from a `proprietary/` path component;
   `DocumentType` from a `papers|reports|standards|benchmarks|manuals/`
   component. Deterministic, so proprietary material can't be mislabelled "open"
   by accident.

Author parsing: `/Author` split on ` and ` / `;`; each name split as
`Family, Given` (comma form) or "last whitespace token is the family name". This
is inherently ambiguous.

**Guiding rule (per task):** unknown fields are left `None`/empty rather than
guessed. **No author guessing from body text at all** — a wrong author/DOI is
worse than a missing one, and a human reviewer fills gaps against the source.

`slug` = `<firstauthorfamily><year><firsttitleword>` lowercased-alphanumeric
(e.g. `doe2021test`), falling back to a slugged title. `id` = deterministic
64-bit **FNV-1a** hash of `slug\0title`, rendered `kovan-<16 hex>` — small,
dependency-free, and stable across re-ingest (no timestamps/randomness).

## BibTeX rendering (`to_bibtex`) — mapping choices

The Rust struct stays authoritative; BibTeX is generated from it (never the
reverse), per `docs/kovan.md` "Canonical Representation".

Entry-type map: `Paper→@article`, `Report→@techreport`, `Manual→@manual`,
`Standard/Benchmark/Other→@misc`. `@misc` is the portable fallback — classic
BibTeX has no `@standard`/`@benchmark` and `@misc` accepts arbitrary fields, so
nothing is lost (biblatex users can post-process the type).

Fields emitted when present: `author` (`Family, Given and …`), `title`, `year`,
`journal`, `institution`, `publisher`, `doi`, `url` (from `source_url`),
`keywords`, `abstract`. Absent optionals are omitted. Field order is fixed →
deterministic output. Values are TeX-escaped (`\ { } & % $ # _ ~ ^`), key
sanitised to `[A-Za-z0-9:_-]` (empty → `unknown`). A golden test pins the exact
`@techreport` output.

## New dependencies added (root `[workspace.dependencies]`)

| Dep | Version | Licence | Why |
|---|---|---|---|
| `pdf-extract` | 0.12 | MIT | Deterministic pure-Rust PDF text extraction (offline, Android-clean). |
| `lopdf` | 0.42 | MIT | Low-level PDF object model — Info-dict metadata + raw image assets; also used to synthesise the test PDF. |

Both inherited via `.workspace = true`; no existing version was changed. They
are pulled only by `kovan-literature`, so no other crate's build is affected.

## Testing

25 unit tests, all offline and synthetic (no real/proprietary fixtures). The
test PDF is built in-memory with `lopdf` under `#[cfg(test)]` (`src/test_pdf.rs`)
— text body + Info dict — and the pipeline is exercised end-to-end
(`pdf_to_markdown` decodes "Kovan Steam Tables Report"; `extract_metadata`
recovers title/authors/year/keywords from the Info dict). Plus a BibTeX golden
test, escaping test, Markdown outline/paragraph/heading/split tests, DOI/date/
author/slug/id unit tests.

## What is implemented vs. stubbed

- **Implemented & tested:** `extract_pdf_text`, `pdf_to_markdown`,
  `text_to_markdown`, `split_markdown_by_page_limit`, `markdown_outline`,
  `extract_metadata`, `to_bibtex`, storage `visibility_from_path` /
  `document_type_from_path`.
- **Partial (documented):** `extract_assets` — only DCTDecode/JPXDecode images
  (see above).
- **Stubbed:** none. (The former `LiteratureError::Unimplemented` variant is
  retained in the public enum for future use but is no longer returned by any
  pipeline function.)

## Suggested additions to `kovan-common` (NOT edited — reported per task)

These would let the pipeline populate fields it currently has to drop:

- `KovanDocument` has no field for **page count**, **source file path/hash**, or
  a **generated-Markdown path / chunk list** — the pipeline computes a body but
  cannot record where it was split or which asset files belong to it. Consider a
  `assets: Vec<String>` and/or `page_count: Option<u32>`.
- No **`conference`/`volume`/`pages`/`number`/`month`** fields, so `@article`
  entries can't carry full journal locators. Add if richer BibTeX is wanted.
- `Author` has no ORCID; fine for now.
- A `KovanDocument::builder()` (or a `from_parts`) would be cleaner than the
  current field-by-field mutation the pipeline does after `::new`.

None of these block the current pipeline; they are the natural next fields.

## Open questions for human review

1. **Asset re-encoding:** is DCTDecode/JPXDecode-only acceptable long-term, or
   should we add a pure-Rust PNG encoder path for `FlateDecode` image samples?
   (Would add an `image`/`png` dep — check Android build first.)
2. **Metadata trust:** the text-fallback year is deliberately low-precision.
   Should ingest instead leave `year=None` when the Info date is absent, forcing
   a human to fill it? Current code prefers a best-effort guess.
3. **`document_type` default:** unknown path → `DocumentType::Other`. Confirm
   that's preferred over defaulting to `Paper`.
4. **BibTeX `abstract` field:** included when present (biblatex-friendly, classic
   BibTeX ignores it). Confirm that's wanted.

## Intended beads (KOVAN epic `op-5v5` is JSONL-only / not in local Dolt)

Per the task, recorded here instead of created (do not touch the broken export):

- **op-5v5.C1** — Asset extraction: add pure-Rust PNG re-encode for FlateDecode
  image XObjects (Android-gated dep check first). [enhancement]
- **op-5v5.C2** — Richer metadata: pull `journal`/`volume`/`pages` via text
  heuristics or a CrossRef-offline map; needs new `KovanDocument` fields.
- **op-5v5.D1** — Bibliography (multi-entry) writer: emit a full `.bib` file from
  a `Vec<KovanDocument>` into `generated/bibtex/`, with de-duplicated keys.
- **op-5v5.common1** — Add `assets`/`page_count` (and optional journal locator)
  fields + a builder to `kovan-common::KovanDocument` (see above).

## Collation for the citeproc port (GitHub #795, epic #790) — 2026-10-08

**Decision: `icu_collator` (ICU4X) 2.x, behind `citeproc::js::locale_compare`.**
citeproc-js sorts and compares with node's ICU (`String.prototype.localeCompare`;
sort.js builds its comparator with `{sensitivity: "base", ignorePunctuation: true,
numeric: true}`; build.js probes `'dale|'.localeCompare('daleb', locale)` to choose
`sort_sep`; util_substitute.js tests `!a.localeCompare(b)`). The port needs the same
order, not a similar one: a different tie-break reorders a bibliography or a
year-suffix run.

**What was built.** `js::locale_compare` (default options), `js::locale_compare_sort`
(the sort comparator's options) and `js::locale_compare_with`, the single place every
comparison goes through. One `CollatorBorrowed` is cached per `(locale, options)`.
`sensitivity: "base"` is `Strength::Primary`, `ignorePunctuation` is
`AlternateHandling::Shifted`, `numeric` is the `CollationNumericOrdering::True`
preference; an unparsable tag collates as root (node throws a RangeError there).
`build.rs`'s `sort_sep` now runs the real `'dale|'.localeCompare('daleb', ...)` probe
(it was a hardcoded `"|"`); the answer is `"|"` for all 16 locales the suite names.

**Why this one.**

| Candidate | Verdict |
|---|---|
| `icu_collator` 2.3 (ICU4X) | **Chosen.** Pure Rust, compiled CLDR data (CLDR 48.2.1, ICU release-78.1rc) against node 22.22.2's ICU 78.2 / CLDR 48, so the root order and the tailorings are the same release. Already in `Cargo.lock` (via ~~`hayagriva` and~~ **CORRECTED 2026-10-08 (#797): hayagriva is removed; `turso_core` still pulls it in**), so no new crate enters the tree. `cargo check --release --target wasm32-unknown-unknown` is clean. |
| the previous stand-in (NFD, strip marks, lowercase, compare) | Rejected: no punctuation or digit handling, no tailorings, and it ordered `|` after letters. |
| `rust_icu` / ICU4C bindings | Rejected: C library, not buildable for Android/Termux or wasm. |
| a DUCET/CLDR-root-only crate | Rejected: no locale tailorings (da, ro, el, ar, zh, km occur in the suite). |

**Licence.** `icu_collator` and `icu_collator_data` are `Unicode-3.0` (SPDX; the licence
text is the crate's `LICENSE`: "UNICODE LICENSE V3", read 2026-10-08). It is permissive:
the only condition is to keep the copyright and permission notice with copies of the data
files or software, and there is no copyleft, advertising clause or field-of-use limit, so
it is compatible with this crate's AGPL-3.0-only. (It is also the licence of the Unicode
data in the `icu_*` crates the tree already carries.)

**Measured agreement** (`js::collation_tests::locale_compare_agrees_with_node_icu`, data
`tests/data/csl/units/collation.json`, generator `scripts/csl-units/collation.cjs`):
10,030 strings (every string value of the fixtures' INPUT items and of `items.json`;
upper/lower/NFC/NFD/accent/bracket/quote/hyphen/trailing-`|`/digit variants; sort-key-shaped
`word|word|` forms; a hand list of hard cases: `ß æ ø ł ñ ǆ ﬁ İ ı`, Greek, Cyrillic, Arabic,
Thai, Khmer, kana, CJK, an emoji, zero-width and soft-hyphen characters, `van der`/`O'`/`Mc`
names) in 18,004 pairs (every neighbour pair of node's sorted order, plus all pairs of the
hand list) under the 16 locales the suite's styles name (`en-US en-GB en fr-FR fr fr-CA
de-DE da-DK ro-RO pt-BR el ar zh-TW km-KH gx en-US-x-sort-ja-alalc97`) with both option
sets: **576,128 comparisons, 0 mismatches (100 %)** against node v22.22.2 (ICU 78.2,
Unicode 17.0, CLDR 48). The test fails on any difference.

**Limits.** The reference is node 22.22.2; a different node/ICU release may differ in
tailorings that changed between CLDR releases. Fixtures compare through citeproc-js's
`toLocaleLowerCase` first (`load::to_locale_lower_case`), which is not part of this
decision. The data adds a few MB to the binary.
