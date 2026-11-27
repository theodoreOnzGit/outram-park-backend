# kovan-literature

**KOVAN** — **K**nowledge **O**riented **V**&V **A**nalysis for **N**uclear
**S**ciences. This crate is part of KOVAN: its literature archive
and the PDF-to-BibTeX pipeline.

> **On crates.io (2026-10-07):** the KOVAN app is published as
> `knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan`, its backronym
> spelled out (`kovan` on crates.io is an unrelated crate). Install it with
> `cargo install knowledge-oriented-vv-analysis-for-nuclear-sciences-kovan`;
> the binaries are still `kovan`, `kovan-cli` and `kovan-tui`, and the library
> is still `kovan`.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

**Scope.** The literature side of OUTRAM PARK: the archive of open
documents, the PDF-to-BibTeX pipeline, the Zotero and citeproc-js ports that
read, search and cite it, and the concept tree that files nuclear knowledge
under the IAEA's 19 infrastructure issues. **The one question it answers:**
what does the literature say, where is it, and which concept does it
support?

**Knowledge it makes studyable.** This crate is where a reader starts from
a paper. It keeps each open document's metadata and extracted text, the
datasets digitised from their figures (`derived/`, each with its source,
figure and method), and a concept tree whose every node cites the document
and section it comes from, so a reader can check the tree against
NG-G-3.1 or NUREG-0800 and challenge any node. The citeproc-js and Zotero
ports can be read beside their upstream JavaScript, with every deviation
registered (`src/citeproc/DEVIATIONS.md`).

**Status:** AI draft (`maturity = 1` in `Cargo.toml`), not human-reviewed.
The concept tree's levels 1-3 were approved by the maintainer (2026-10-06,
below).

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | [`CATALOGUE.md`](CATALOGUE.md) records every document; the open PDFs are in the `reactor-literature/` submodule (`kovan-standard-open-corpus/`, `theodore-open-corpus/`); metadata JSON and extracted Markdown under `open/`, BibTeX under `generated/`. The concept tree cites 59 `[[document]]`s (`src/concept_skeleton.toml`), among them IAEA NG-G-3.1 (Rev. 1) and NUREG-0800. Ported upstreams (commits in [`NOTICE`](NOTICE)): Zotero `9cbba8c4d281`, utilities `4051881d59c6`, translators `3d1c78530f42`, translate `e0fe482b8a07`/`dd524aea9a55`, translation-server `3a9d17614896` (AGPL-3.0); citeproc-js 2.4.63 (AGPL-3.0 option). |
| Mathematical equations | None of its own. Digitised datasets under `derived/` carry the quantities they were read off, e.g. the BSAF Cs-137 release-rate histories (Bq/hr against days) from OECD/NEA No. 7525, Figures 5.1-5.4. |
| Source code | `src/pdf_import.rs`, `metadata.rs`, `markdown.rs`, `bibtex.rs`; `concept_tree.rs` with `concept_skeleton.toml`, `concept_proposals.toml`, `concept_sciences_proposals.toml`; `zotero/` (`framework`, `translators`, `local_library`, `search`); `lookup_review.rs`; `citeproc/`; `csl.rs`. |
| Interactive simulations | None yet. The concept tree is drawn as Kovan's standard mind map (`crates/kovan`). CLI: `kovan-cli lit`; example `examples/bibtex.rs`. |
| Visualisations | [`digitiser_demo_loglog.png`](digitiser_demo_loglog.png); digitised-figure overlays such as `derived/nea7525-bsaf-unit1-figures/fig2.1a-digitised.png`; the generated review documents [`docs/concept-proposals.md`](docs/concept-proposals.md) and [`docs/concept-sciences-proposals.md`](docs/concept-sciences-proposals.md). |
| Verification and validation evidence | `tests/citeproc_test_suite.rs` (2026-10-08: of 845 CSL fixtures the port equals citeproc-js's output on 832 and the fixture's `RESULT` on 842; the 13 others differ from citeproc-js only through newer locales); `tests/csl_vs_citeproc_js.rs` (2026-10-08: 0 differences over 219 items × 3 parts × 5 styles = 3,285 comparisons); `tests/zotero_translators*.rs`, `tests/zotero_xml_translators.rs` (against a running Zotero translation-server's committed outputs); `tests/zotero_search*.rs`; `tests/zotero_local_library.rs`; `tests/concept_skeleton.rs`, `concept_proposals.rs`, `concept_sciences.rs`; `tests/bsaf_cs137_release_rates.rs` (regression guard on 10 403 digitised points, #1028). |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md`. The concept tree's levels 1-3 carry the maintainer's approval (2026-10-06); 139 of 147 Sciences-tree nodes are approved (`docs/concept-sciences-proposals.md`). Corrections to the record are struck through in [`CATALOGUE.md`](CATALOGUE.md) (for example the moves of the JRC and NJOY2016 documents) and in this README. |

The citeproc-js and Zotero ports have code-to-code evidence against upstream
output; the concept tree has the maintainer's approval but no reviewed code;
the PDF pipeline is unit-tested and best-effort, and nothing is
human-reviewed.

## What the library does

The library turns a source PDF into the canonical `KovanDocument` (from
`kovan-common`) and derives Markdown, BibTeX and extracted image assets from
it:

```text
PDF → Markdown → KovanDocument → BibTeX → generated knowledge artifacts
```

The `KovanDocument` is authoritative; BibTeX and generated Markdown are always
derived from it. Every function is deterministic and runs fully offline, with
no network, cloud or OCR service. Text extraction uses the pure-Rust
`pdf-extract` crate and the PDF object model uses `lopdf`. Both build for
`aarch64-linux-android`.

## What is implemented and what is best-effort

Per `src/lib.rs`:

| Function | Status |
|---|---|
| `pdf_to_markdown`, `markdown_outline`, `to_bibtex` | Implemented and tested |
| `extract_metadata` | Best-effort: the PDF Info dictionary first, then conservative text scanning. Unknown fields are left empty rather than guessed. |
| `extract_assets` | Extracts embedded images already stored as standalone files (JPEG via `DCTDecode`, JPEG-2000 via `JPXDecode`). Images under other filters are reported as skipped, not re-encoded. |
| `zotero::local_library` | Reads a whole Zotero data folder (`zotero.sqlite` + `storage/`, Zotero 5.0+) read-only through a private in-memory copy, into `kovan_common::zotero`'s model, and imports it as `KovanDocument`s with a report of what is lost (GitHub #750). `read_database_files` reads the database from bytes, with no file system (a browser). AI draft, verified code-to-code against databases built from Zotero's own schema files, and its SQLite layer against real SQLite. ~~**Native desktop only:** SQLite is compiled from C, so this module is not built for wasm32 or Android.~~ **CORRECTED 2026-10-07:** pure Rust (`turso_core`, a Rust rewrite of SQLite, MIT); builds for every target, wasm32 and Android included. |
| `zotero::search`, `lookup_review` (since 2026-10-07) | Zotero's "Add Item by Identifier" (#756): `extractIdentifiers` (DOI, ISBN, arXiv, ADS bibcode, PMID) and the 19 search translators (arXiv, DOI Content Negotiation, Crossref REST, PubMed, the ISBN catalogues, ...), with Zotero's fallback order. **Network-free:** a lookup returns the HTTP request it needs and the caller fetches it (kovan's `lookup_net` natively; a browser with `fetch`), so the crate stays wasm32- and Android-clean; every failure is a typed `LookupError`. `lookup_review` compares a fetched record with an ingested PDF's metadata field by field for the user to confirm. Verified code-to-code against upstream run on the same recorded responses (`scripts/zotero-search-reference.mjs`, `tests/zotero_search.rs`, `tests/zotero_search_failures.rs`). AI draft, not yet human-reviewed. |
| `zotero` (since 2026-10-07) | Port of Zotero's translation framework and its import/export translators (#749): BibTeX, BibLaTeX, RIS, CSL JSON; the tagged-text, JSON and simple export translators; the XML translators (MODS, Endnote XML, TEI, Crossref Unixref XML, MARCXML, MARC, PubMed XML, METS, Primo Normalized XML, DSpace Intermediate Metadata, Citavi 5 XML, XML ContextObject) on an XML DOM and XPath engine; Note HTML and Note Markdown. Compared with a running Zotero translation-server on committed reference outputs (#752; `scripts/zotero-reference.sh`, `tests/zotero_translators.rs`, `tests/zotero_xml_translators.rs`). AI draft, not yet human-reviewed. |
| `citeproc`, `csl` (since 2026-10-08) | `citeproc`: a port of citeproc-js 2.4.63, Zotero's CSL engine (#790; AGPL-3.0 option of citeproc-js's licence, see [`NOTICE`](NOTICE)); ~~845/845 CSL test-suite fixtures and the site's 223 entries under five styles equal citeproc-js's output.~~ **CORRECTED 2026-11-27** (read from the doc comments of `tests/citeproc_test_suite.rs` and `tests/csl_vs_citeproc_js.rs`, not re-run): of the 845 fixtures the port equals citeproc-js's output on 832 and the fixture's own `RESULT` on 842; the other 13 differ from citeproc-js because the port uses current CSL locales, and each equals its `RESULT`. The site's references (219 entries since four duplicates were removed) under five styles show 0 differences from citeproc-js (2026-10-08). `csl`: citations and reference lists for the Pages site's markdown and HTML on top of it (#789, #797; replaced hayagriva, which differed from citeproc-js in 176 of 669 comparisons). Not authoritative for any publication; verify against the style. |

The graph digitiser is **not** in this crate. It moved to `kovan` on
2026-08-21 (`kovan::digitiser`) so this crate could stay GPL-3.0-only
(~~stay GPL-3.0-only~~ **2026-10-07:** this crate is AGPL-3.0-only too, like all
of kovan; see [`NOTICE`](NOTICE). The digitiser has not moved back).

## The archive

This crate is also the on-disk literature archive. **It holds no PDFs.** Since
2026-09-22 the open PDFs are in the `reactor-literature/` Git submodule
(fetch it with `git submodule update --init crates/kovan-literature/reactor-literature`),
and proprietary documents are kept outside this public repository. The crate
keeps each open document's metadata JSON and extracted Markdown under `open/`,
generated Markdown and BibTeX under `generated/`, and datasets extracted from
documents, with their provenance, under `derived/`.

- [`CATALOGUE.md`](CATALOGUE.md) is the record of every document: what it
  is, its access tier and why, and where its files live.
- ~~[`CLAUDE.md`](CLAUDE.md)~~ [`AGENTS.md`](AGENTS.md) holds the rules for adding or moving a document.
  The access tier comes from the document's own licence, never from where it
  is hosted (workspace `DATA_POLICY.md`).

Only `generated/bibtex/open/` ships in the published crate. The PDFs,
Markdown, `derived/` data and the submodule are repository-only (see the
`exclude` list in `Cargo.toml`).

## The concept tree: four levels

**Decided by the maintainer on 2026-10-06 (GitHub #724, #726, #727, #729).**
Kovan organises nuclear knowledge in one concept tree, shared by its two
master tabs: **Literature** (the default) and **Code Review**. Every node is
a concept; code never appears as a node.

| Level | What it is | Where it comes from | Path segment | Who changes it |
|---|---|---|---|---|
| **1** | The **19 IAEA infrastructure issues** | IAEA Nuclear Energy Series NG-G-3.1 (Rev. 1), *Milestones in the Development of a National Infrastructure for Nuclear Power*, 2015, §3.1–3.19 | numbered so `NN` is §3.NN: `02-nuclear-safety` | frozen |
| **2** | **Regulatory review categories**: the chapters a regulator reviews under each issue | NUREG-0800 (SRP), NUREG-1537 Part 1, NUREG-1520, NUREG-1555, NUREG-0654/FEMA-REP-1, RG 1.232, ORNL/TM-2018/976 (MSR), 10 CFR 50/52/53; where the NRC is silent, the Milestones text itself | plain words, no number: `nuclear-design` | the maintainer, rarely |
| **3** | **Concepts**, nesting as deep as needed | the sub-sections of those documents, plus concepts outram-park's code needs that they only imply (`origin = "outram-park"`, each with a `why`) | plain words: `neutron-transport/delta-tracking/majorant` | agents propose, the maintainer approves; backwards compatible once published |
| **4** | **Implementations**: which code implements a concept | generated by kovan from `kovan-concept: <path>` lines in each module's doc comment; world implementations (OpenMC, NJOY2016, GeN-Foam, …) listed first and credited, outram-park's after, each with its maturity rung | crate and module | the code itself; bleeding edge, follows `develop` |

A full path reads:

```text
02-nuclear-safety/nuclear-design/neutron-transport/delta-tracking/majorant
└ L1 (IAEA §3.2)  └ L2 (SRP 4.3,    └ L3 concept ……………………………
                    NUREG-1537 4.5)
```

**Rules that bind the tree**

- **Every node cites its source** by document and section (and page where
  the document gives one). A source names a `[[document]]` whose file is in
  the standard corpus (`reactor-literature/kovan-standard-open-corpus/`) or,
  for documents that may not be redistributed (the IAEA ones), the
  maintainer's private corpus, cited only.
- **One home, many links.** Each concept has one home path; `cross_links`
  (dotted on the map) name every other node it belongs to. An
  **aspiration** link (dashed) marks what outram-park work aims to support but
  is not validated, risk-assessed or licensed for, with its prerequisites
  stated.
- **Empty is greyed, not hidden.** A node with nothing classified under it is
  greyed; a "show empty nodes" toggle hides or reveals them. "Greyed" is per
  tab: no literature in Literature, no outram-park implementation in Code
  Review.
- **outram-park appears in Literature only through peer-reviewed
  publications** (today: TUAS, Ong, Xiao & Peterson 2025,
  doi:10.1016/j.jandt.2025.03.006), which enter the standard corpus. Unpublished
  work is visible only in Code Review.
- **Implementation maturity** (level 4) is shown as one of ~~four~~ five rungs:
  (1) AI translated or constructed, (2) AI V&V, (3) human reviewed,
  (4) human V&V (a human builds a code-to-code verification or validation case
  through the API or GUI), **(5) independent V&V (IV&V)** (**CORRECTED
  2026-11-27**, as `crates/kovan/README.md` already records since 2026-10-08,
  GitHub #809: the review engine derives rung 5; rules in
  `kovan_common::review::ivv`). Handwritten code starts at rung 3.
- **Compatibility.** Levels 1–2 change rarely; level 3 never breaks a
  published path (a move leaves an alias, a deletion becomes `deprecated`);
  level 4 follows the code and is regenerated, with a staleness check.

**Where it lives (in this crate, so the browser build of Code Review can use
it: this crate builds for `wasm32-unknown-unknown`)**

| File | Holds | Checked by |
|---|---|---|
| `src/concept_skeleton.toml` | levels 1–2 and the `[[document]]` list | `tests/concept_skeleton.rs`: 19 L1 nodes in IAEA order, unique paths, parents exist, L2 unnumbered, every source a declared document, every standard-tier file present in the corpus, every cross-link resolves |
| `src/concept_proposals.toml` | level-3 concepts (`status`: proposed / approved / deferred) and the seed `kovan-concept` tags for level 4 | `tests/concept_proposals.rs`: parents and sources exist, origins valid, cross-links and aspirations resolve, every tagged module exists on `develop` |
| `docs/concept-proposals.md` | the review document, generated from the TOML (`KOVAN_REGENERATE_PROPOSALS=1 cargo test --release -p kovan-literature --test concept_proposals`) | the same test fails if it is stale |

**Status (2026-10-06).** Levels 1–3 are reviewed and approved: 19 issues,
89 level-2 categories and ~~263 concepts~~ (3 deferred awaiting sources), with
over 90 cross-links. **CORRECTED 2026-11-27** — `src/concept_proposals.toml`
now holds 309 `[[concept]]` entries (306 `approved`, 3 `deferred`) and 192
proposed `[[implementation]]` entries; the skeleton still has 108 nodes
(19 + 89). Counted in the TOML files; cross-links not re-counted. ~~**Kovan does not read the tree yet**~~ **Since
2026-10-06 Kovan's standard mind map is levels 1–3 of this tree**, replacing
the 44 hand-written `nuclear-engineering/...` topics: `kovan-literature`'s
`concept_tree` module parses both TOML files once into a typed `ConceptTree`,
and Kovan's `corpus.rs` draws it under one virtual root, "Nuclear knowledge
(IAEA Milestones)" (id `corpus:concept/iaea_milestones`), in the map's
existing style. Node ids are the concept paths themselves, so a `kovan.toml`
classification such as `02-nuclear-safety/nuclear-design/neutron-transport`
resolves directly; cross-links are drawn as built-in, read-only link cards;
deferred concepts are in the tree and flagged. Every `[[document]]` the tree
cites is a standard-corpus literature entry (its metadata is compiled into
Kovan; the PDF opens from the `reactor-literature` checkout when one is
present), the two IAEA ones as citation-only entries, so the map needs
neither the corpus checkout nor the private repository. A user's own
`topics/` folders (including old `nuclear-engineering/...` ones) keep working
as the user's topics. Still to do: ~~the Code Review tab,~~ level-4 generation
from code tags, the drift report, the maturity rungs (#729), the "show empty
nodes" toggle (the model's `corpus::has_literature` /
`has_classified_literature` are the hook), and source hyperlinks on cards. **CORRECTED 2026-11-27** — desktop Kovan has
a Code Review tab (`View::CodeReview` in `crates/kovan/src/app/nav.rs`).
The other items were not re-checked (the per-crate `maturity` field of
`[package.metadata.kovan]` exists, but whether level-4 nodes show rungs was
not checked);
`crates/kovan/src/corpus.rs` still calls `has_literature` the hook for the
"show empty nodes" toggle.

## Example

```bash
cargo run -p kovan-literature --release --example bibtex
```

From the CLI: `kovan-cli lit`. The public API mirror is
[`docs/kovan-literature-api.md`](docs/kovan-literature-api.md).

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

~~GPL-3.0.~~ **AGPL-3.0-only since 2026-10-07**, like all of kovan (see [`NOTICE`](NOTICE)). Part of the [OUTRAM PARK](../../README.md) workspace.
