# References — `open/theses/`

Provenance for the theses in this directory, per the workspace `CLAUDE.md`
data-provenance rule and `RESPONSIBLE_USE.md`. Every entry records its source,
author, title, access terms, permalink, date accessed, and the processing steps
that produced the derived artifacts.

All three are open-access dissertations deposited in **eScholarship**, the
University of California's open-access repository (California Digital Library).
Nothing here is NUS Confidential/Restricted, partner-confidential, operational,
or unpublished third-party research data.

## Documents

### `wang2018coupled.pdf`

| Field | Value |
|---|---|
| Author | Wang, Xin |
| Title | Coupled neutronics and thermal-hydraulics modeling for pebble-bed Fluoride-Salt-Cooled, High-Temperature Reactor (FHR) |
| Institution | University of California, Berkeley |
| Publication date | 2018 |
| Type | PhD dissertation |
| Permalink | https://escholarship.org/uc/item/40q3985m |
| Pages | 137 |
| Date accessed | 2026-07-30 |
| Original filename | `Xin Wang Thesis.pdf` |

Citation of record, supplied by the maintainer:

> Wang, X. (2018). *Coupled neutronics and thermal-hydraulics modeling for
> pebble-bed Fluoride-Salt-Cooled, High-Temperature Reactor (FHR)*. University
> of California, Berkeley.

**Source note.** An earlier copy of this PDF, replaced on 2026-07-30, had no
eScholarship cover page and its title page read "Spring 2014". The copy archived
here is the eScholarship deposit, whose cover page states `Publication Date
2018`, matching the citation of record. The 2018 date is used throughout. Anyone
comparing against the earlier file should expect the different date and a
slightly different byte count.

### `alivisatos2023evaluating.pdf`

| Field | Value |
|---|---|
| Author | Alivisatos, Clara |
| Title | Evaluating Remote Operations for Advanced Nuclear Reactor Control: Feasibility, Benefits, and Implementation Criteria |
| Institution | University of California, Berkeley |
| Publication date | 2023 |
| Type | PhD dissertation |
| Permalink | https://escholarship.org/uc/item/1wt929p1 |
| Pages | 106 |
| Date accessed | 2026-07-26 |
| Original filename | `alivisatos thesis.pdf` |

### `poresky2019model.pdf`

| Field | Value |
|---|---|
| Author | Poresky, Christopher Morris |
| Title | Model Network Methodology for Experimental Development of Industrial Monitoring Systems |
| Institution | University of California, Berkeley |
| Publication date | 2019 |
| Type | PhD dissertation |
| Permalink | https://escholarship.org/uc/item/9bz6h8d2 |
| Pages | 154 |
| Date accessed | 2026-07-26 |
| Original filename | `poresky thesis.pdf` |

## Access terms

All three are open-access eScholarship deposits, marked
`Peer reviewed | Thesis/dissertation` on their cover pages, and the maintainer
has confirmed they are open. That is the basis for storing them under `open/`
and committing them to the repository.

**Not yet verified per item:** the *specific* licence each author selected on
deposit (eScholarship items may be CC-BY, another Creative Commons variant, or
"all rights reserved with open access to read"). The deposit PDFs do not state a
licence in their text, so no licence is asserted here rather than guessing one.
Redistribution beyond this repository — including publishing these PDFs to
crates.io — should not be assumed permitted until each item's licence is checked
against its permalink. This is why `crates/kovan-literature/Cargo.toml` excludes
`open/` from the packaged crate while still committing it to GitHub; only the
generated BibTeX (bibliographic facts, not copyrightable expression) is
published.

## Derived artifacts and how they were produced

| Artifact | Location |
|---|---|
| Markdown body | `generated/markdown/open/<slug>.md` |
| BibTeX entry | `generated/bibtex/open/<slug>.bib` |

Produced on **2026-07-30** with the `kovan` CLI at commit-time workspace state,
release build:

```bash
kovan lit import <pdf> --markdown-out generated/markdown/open/<slug>.md
kovan lit bibtex <pdf>              >  generated/bibtex/open/<slug>.bib
```

Both steps are deterministic and fully offline (`pdf-extract` for text,
`lopdf` for the PDF object model; no OCR, no network). Re-running them on the
same PDF reproduces the same bytes.

Metadata was **not** hand-entered: the author, title, year and permalink above
are parsed from each deposit's labelled eScholarship cover page by
`kovan_literature::extract_metadata`. Page counts come from the PDF page tree
(`/Pages` `/Count`).

## Known limitations of the derived markdown

Recorded so a reader does not mistake these files for clean full text:

1. **Not split by page.** `docs/kovan.md` targets 30 source pages per generated
   Markdown document (`MAX_MARKDOWN_PAGES`). `pdf-extract` emits no form-feed
   page separators for these deposits, so `split_markdown_by_page_limit` has no
   boundaries to split on and each thesis is one 200-310 KB body. The page
   *counts* above are correct (read from the page tree); the markdown is simply
   unsegmented.
2. **Equation and figure text is degraded.** The converter is a text extractor,
   not a layout engine. Mathematics comes through as broken fragments — the
   heading outline for `wang2018coupled` contains artefacts such as
   `## 0 d E ′ ∫` picked up from displayed equations. Figures and tables are not
   reconstructed; embedded raster images are not extracted into
   `generated/assets/` for these documents.
3. **Letter-spaced cover text.** The Alivisatos deposit's title page extracts
   with per-character spacing (`E v a l u a t i n g …`). The cover-page metadata
   is unaffected, since it is parsed from the labelled fields.
4. **No `school` field in the BibTeX.** `extract_metadata` does not recover the
   awarding institution, so the generated `@phdthesis` entries carry no `school`.
   The institution is recorded in this file and should be added to the entries
   when institution extraction lands.

These artifacts are AI-assisted derived output and, per `RESPONSIBLE_USE.md`,
remain untrusted draft material until a human reviews them against the source
PDFs.
