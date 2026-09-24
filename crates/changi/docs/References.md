# References — `changi`

Provenance for every external number this crate's examples and illustrative
data depend on, per the workspace `CLAUDE.md` "Responsible use & data policy"
rule: source, organisation, title, access terms, URL, date accessed, and any
processing or assumptions applied.

This file covers **data**. Ported *code* provenance lives in `LICENSE.flexpart`
/ `NOTICE.flexpart` and `LICENSE.puff` / `NOTICE.puff`.

---

## Singapore surface wind climatology

Used by `src/puff/climatology.rs` and, through it, by all three examples.

| Field | Value |
|---|---|
| Organisation | Meteorological Service Singapore (MSS), a division of the National Environment Agency |
| Title | *Climate of Singapore* |
| URL | <https://www.weather.gov.sg/climate-climate-of-singapore/> |
| Access terms | Public government information page, openly published |
| Date accessed | 2026-09-23 |
| Retrieval | **Indirect — see the caveat below** |
| Status | **Not re-checked against the primary source** |

### Figures taken, and which are ours rather than theirs

| Quantity | Value used | As stated by the source |
|---|---|---|
| Mean surface wind speed | `2 m/s` | "winds are generally light, with mean surface wind speed of around 2 m/s" |
| Northeast Monsoon months | December to early March | as stated |
| Northeast Monsoon direction | from `030` deg | "northerly to northeasterly" — `030` deg is a **mid-sector choice by this project**, not a published mode |
| Southwest Monsoon months | June to September | as stated |
| Southwest Monsoon direction | from `160` deg | "southeasterly to southerly" — `160` deg is a **mid-sector choice by this project** |
| Inter-monsoon months | April-May, October-November | as stated |
| Inter-monsoon character | light and variable | as stated; the nominal `090` deg in the code is a **placeholder**, since these winds are by definition not persistent |
| Monsoon surge speed | `10 m/s` | "during the Northeast Monsoon surge, mean wind speeds can reach up to 10 m/s or more" |
| Windiest season | NE monsoon, strongest January-February | as stated; the `3 m/s` used for the NE monsoon is a **representative choice by this project**, above the 2 m/s annual mean but well below a surge |

**Four of the nine rows are this project's choices, not MSS figures**, and are
marked as such above. A sector description ("northerly to northeasterly") is
not a direction, and turning one into a single bearing is an assumption. They
are adequate for a demonstration and inadequate for anything else.

### Retrieval caveat — read this before citing the numbers

`weather.gov.sg` and `nea.gov.sg` are both **blocked by the network egress
policy of the development environment** these figures were gathered in, so the
MSS page was **not retrieved directly**. The values above come from web-search
summaries that attribute them to that page, cross-checked against a second
search covering the inter-monsoon periods.

That is weaker provenance than a direct read, and it is recorded as such rather
than presented as a citation that was actually followed. Per the workspace rule
that a claim which cannot be checked is marked rather than left standing, every
figure here is **`Not re-checked against the primary source`**.

**To clear this**: open the MSS page directly, confirm each row, and replace
this caveat with the date it was verified. If any figure differs, correct
`src/puff/climatology.rs` and re-run
`cargo run --release -p changi --example puff_site_survey`, whose printed
numbers depend on them.

### Scope limit

These are **illustrative climatological conditions for demonstrations**, not a
site characterisation and not a design basis. A real assessment needs the
site's own measured wind rose at release height, over a defined averaging
period, with a stability joint-frequency distribution. The crate-level scope
limits in `src/lib.rs` are binding and nothing here relaxes them: CHANGI is for
research, education and V&V only.

---

## Pasquill-Gifford dispersion coefficients

The `(a, b, c, d)` coefficient tables in `src/puff/dispersion.rs` are **not**
independently sourced — they are transcribed from the upstream R package
`puff` 0.1.1 (commit `5213d58`) as part of the port, and verified bit-exactly
against it across 234 cases. See `docs/puff-code-to-code.md`.

Upstream attributes the algebraic form to the standard Martin (1976)
parameterisation used by the US EPA's ISC models. **This project has not traced
the coefficients back to that primary source**, and the code-to-code
verification does not check them against it — it checks that the port
reproduces upstream, which is a different claim. Anyone needing the values
themselves to be right, rather than faithfully copied, should verify them
against the primary literature.

---

## Deposition velocities

`src/activity/deposition.rs` carries **uncited order-of-magnitude
placeholders**, as its own documentation states. They are not sourced and must
not be cited. Tracked separately; not resolved by this file.

## HTR-10 equilibrium-core fission-product inventory

`reference/htr10_equilibrium_core_inventory.csv`, exposed by
`changi::activity::inventory`. Added 2026-09-24.

| Field | Value |
|---|---|
| Source | Liu Yuanzhong and Cao Jianzhu, "Fission product release and its environment impact for normal reactor operations and for relevant accidents" |
| Publication | *Nuclear Engineering and Design* **218** (2002) 81–90, Elsevier |
| Affiliation | Institute of Nuclear Energy Technology, Tsinghua University, Beijing |
| Table | Table 1, "fission product inventories for equilibrium core of HTR-10" |
| How the source computed it | ORIGEN2, average burnup 80 000 MWd/t |
| Contents | 22 nuclides, becquerels |
| Copyright | "© 2002 Elsevier Science B.V. All rights reserved." |
| Access terms | **Restricted.** No reuse licence is stated in the document. The PDF is held in the maintainer's private literature repository and is **not** redistributable |
| Date accessed | 2026-09-23 |

### Why the table is reproduced and the document is not

The private corpus's own README draws the line: *"Research knowledge about
them (citations, notes, connections) may still live in public Kovan
libraries; the documents and their extracted full text may not."* A cited
table of 22 published values is `DATA_POLICY.md`'s "public literature data" —
ordinary scientific citation of a journal article — not the document and not
its extracted full text. **Do not add the PDF, or bulk extracted text from
it, to this repository.**

### Not verified here

The values have not been re-checked against the published Table 1 by anyone
in this repository; the transcription is the maintainer's own, made through
kovan's annotation tooling on 2026-09-23. No calculation here reproduces
them.

### An inventory is not a source term

What is *in the core* is not what gets *out*. Converting one to the other
needs a release fraction covering the fuel, the vessel and the building, and
this crate supplies none — that is a reactor and containment question, not a
dispersion one. `changi::activity::inventory` exists so that a caller need
not invent a starting magnitude, not so that one can be quoted as a release.
`RESPONSIBLE_USE.md` applies in full.
