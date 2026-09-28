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

## HTR-10 annual airborne release, normal operation

`reference/htr10_normal_operation_annual_airborne_release.csv`, exposed by
`changi::activity::airborne_release`. Added 2026-09-28. **Reference data only:
nothing in this crate or in `htgr_sim_v1` consumes it.**

| Field | Value |
|---|---|
| Source | Liu Yuanzhong and Cao Jianzhu, "Fission product release and its environment impact for normal reactor operations and for relevant accidents" (citekey `yuanzhong2002fission`) |
| Publication | *Nuclear Engineering and Design* **218** (2002) 81–90, Elsevier |
| Affiliation | Institute of Nuclear Energy Technology, Tsinghua University, Beijing |
| Table | Table 5, "Amount of airborne radioactivity released into the environment in the HTR-10 normal operation conditions", journal p. 85 (PDF page 5) |
| Quantity | Activity released to the environment as airborne effluent, per nuclide |
| Unit and time basis | The table states neither. The paper's text calls it the **annual** amount and gives totals in Bq, so the values are recorded as **Bq released per year of normal operation** (column `annual_release_bq`) |
| Basis of the source's calculation | Cavity-air argon activation, primary-helium leakage, the contaminated-helium tank, fuel-handling vacuum systems, tritiated secondary-steam leakage, maintenance. **Filtration is not credited** (the source calls the calculation conservative) |
| Contents | 22 nuclides, including H-3, C-14 and Ar-41, which are not in the Table 1 inventory. Rb-88 and Sr-90 are in Table 1 but not in Table 5 |
| Copyright | "© 2002 Elsevier Science B.V. All rights reserved." |
| Access terms | **Restricted.** No reuse licence is stated. The PDF is held in the maintainer's private literature repository and is **not** redistributed. Only the cited table of 22 values is reproduced, as ordinary scientific citation |
| Date accessed / transcribed | 2026-09-28 |

### How the values were obtained

~~The maintainer digitised a table from this paper using kovan's table
digitiser on 2026-09-28. The record is labelled "page 5" and was meant to hold
Table 5, but **it holds Table 3** ("Activities of important fission products in
the primary helium ... end of 20a lifetime", journal p. 84), in the form the PDF
text layer produces: superscripts flattened (`2.2×109` for 2.2×10^9), the
stray space in `9. 3×106`, Table 3's 20 nuclides in Table 3's order, and no
C-14 or Ar-41. One entry was also mis-entered relative to Table 3 itself:
`5.4×10e9` for Kr-83m, where Table 3 reads 5.4×10^8. **None of the digitised
values was used.**~~ **CORRECTED 2026-09-28**: that described an earlier save
of the record, which the maintainer has since replaced. The current kovan
artifact `table-5` in the maintainer's notes for `yuanzhong2002fission`
(`[source] page = 5`, `[extraction] method = "pdf_native"`, saved
2026-09-28T02:47:30Z, every value entered by hand by the maintainer with
kovan's table digitiser from the PDF text layer) holds Table 5, and it agrees
with this CSV on **all 22 nuclides and all 22 values** (compared value by
value on 2026-09-28; no differences).

The values therefore rest on two independent readings of the PDF text layer
that agree: the maintainer's kovan digitisation, and a transcription made on
2026-09-28 by an AI agent (Claude, under maintainer direction) from
`pdftotext -f 5 -l 5 -layout`. They were then checked against the totals the paper states in its text:

| Check | Table sum | Stated in text | Difference |
|---|---|---|---|
| All 22 nuclides | 2.004e11 Bq | 2×10^11 Bq | +0.2 % |
| Ar-41 | 1.0e11 Bq | "dominant contributor" | largest entry, consistent |
| H-3 | 7.9e10 Bq | 7.9×10^10 Bq, "second contributor" | exact, and second largest |
| Excluding Ar-41 and H-3 | 2.137e10 Bq | 2.2×10^10 Bq | **−2.9 %** |

The last row does not agree exactly: 2.137e10 rounds to 2.1e10, not 2.2e10.
Every entry was re-read against the text layer and no transcription slip was
found, so the mismatch is in the source. Rounding of the two-significant-figure
entries is a plausible cause, but the paper does not say. It is recorded here
and has not been adjusted. These checks are pinned by
`the_sum_reproduces_the_totals_the_source_states` in `src/activity/airborne_release.rs`.

~~**Not yet verified by a human.** The values were checked against the text
layer and the stated totals, but no one has checked them against the rendered
table. The maintainer should do that, or re-digitise Table 5 with kovan, before
citing them.~~ **CORRECTED 2026-09-28**: the maintainer did digitise Table 5
by hand with kovan, reading the page, and that record matches every value
above.

### Processing

- Notation: the source's `3.8E8` is written `3.8E+08`, matching the Table 1
  CSV's convention. Significant figures are as published (two, or one for
  Sr-89's `8.1`).
- Row order is the source's, which lists Kr-85m before Kr-85, Xe-133m before
  Xe-133 and Xe-135m before Xe-135.
- The source's two-row block layout (11 nuclides per block) was flattened to
  one row per nuclide.
- No unit conversion. The only interpretation is the time basis ("per year"),
  which comes from the paper's text, as described above.

### A normal-operation release is not an accident source term

This table is routine, unfiltered, conservative effluent over a year. The same
paper's accident releases (Table 8) are a different quantity ~~and are not
digitised here~~ (**CORRECTED 2026-09-28**: now stored separately; see the
Table 8 section below). `RESPONSIBLE_USE.md` applies in full: this is not a release
figure for HTR-10 or any other plant for any operational, licensing or safety
purpose.

## HTR-10 primary-helium activity, end of a 20-year life

`reference/htr10_primary_helium_activity_end_of_life.csv`, exposed by
`changi::activity::primary_helium`. Added 2026-09-28. **Reference data only:
nothing in this crate or in `htgr_sim_v1` consumes it.**

| Field | Value |
|---|---|
| Source | Liu Yuanzhong and Cao Jianzhu, "Fission product release and its environment impact for normal reactor operations and for relevant accidents" (citekey `yuanzhong2002fission`) |
| Publication | *Nuclear Engineering and Design* **218** (2002) 81–90, Elsevier |
| Affiliation | Institute of Nuclear Energy Technology, Tsinghua University, Beijing |
| Table | Table 3, "Activities of important fission products in the primary helium of the HTR-10 at the end of 20a lifetime of full power operation (Bq)", journal p. 84 (PDF page 4) |
| Quantity | Activity circulating in the primary helium, per nuclide, at the end of 20 years of full-power operation. Unit (Bq) and basis are stated in the table title (column `primary_helium_activity_bq`) |
| Basis of the source's calculation (paper Section 2.4.1) | Helium purification efficiencies of 99 % for I, Kr, Xe, C and tritium and 90 % for Sr, Ag, Cs and Rb (set conservatively, per the source); plate-out per cycle of 30 % for Rb and Sr, 50 % for Ag and Cs, 20 % for iodine; primary-helium leakage of 1 % of the volume per day. Method: the authors' earlier reference (Liu Yuanzhong, 1994) |
| Contents | 20 nuclides: 10 noble gases, 5 iodines, Sr-89, Cs-134, Cs-137, Ag-110m, H-3. **No C-14** (the text states a primary-helium C-14 total of 6.3×10^4 Bq, but the table does not list it), no Ar-41, no Rb-88, no Sr-90 |
| Copyright | "© 2002 Elsevier Science B.V. All rights reserved." |
| Access terms | **Restricted.** No reuse licence is stated. The PDF is held in the maintainer's private literature repository and is **not** redistributed. Only the cited table of 20 values is reproduced, as ordinary scientific citation. No prose from the paper is copied |
| Date accessed / digitised | 2026-09-28 |

### How the values were obtained and verified

The maintainer digitised the table on 2026-09-28 with kovan's table digitiser
(GUI table grid, every value entered by hand). The kovan record is
`id = "table-3"`, `kind = "digitised_table"`, `[source] page = 4`,
`[extraction] method = "pdf_native"`, saved 2026-09-28T02:54:31Z, in the
maintainer's notes for `yuanzhong2002fission`. It holds the table as two
Nuclide/Activity row pairs of 10 nuclides each.

An AI agent (Claude, under maintainer direction) transcribed Table 3
independently on 2026-09-28 from `pdftotext -f 4 -l 4 -layout` and compared it
value by value with the kovan record. **They agree on all 20 nuclides, all 20
values, and the order.** No discrepancy and no correction. (The earlier,
since-replaced kovan save described under the Table 5 section above had
Kr-83m as `5.4×10e9`; the current `table-3` record has 5.4e8, which matches
the text layer.)

The values were then checked against the primary-helium totals the paper
states in its text (end of Section 2.4, p. 84):

| Group | Table sum | Stated in text | Difference |
|---|---|---|---|
| Noble gases (10 Kr/Xe entries) | 1.3838e10 Bq | 1.4×10^10 Bq | −1.2 % |
| Iodine isotopes (I-131 to I-135) | 4.851e8 Bq | 4.9×10^8 Bq | −1.0 % |
| Long-lived solid isotopes (Sr-89, Cs-134, Cs-137, Ag-110m) | 2217.9 Bq | 2.2×10^3 Bq | +0.8 % |
| Tritium | 5.7e9 Bq | 5.7×10^9 Bq | exact |
| C-14 | not tabulated | 6.3×10^4 Bq | not checkable |

All four checkable groups agree to within two-significant-figure rounding. The
paper does not say which nuclides its "long-lived solid" group contains; the
four non-gaseous entries were used, and Sr-89 (1.9 Bq) cannot move the sum
either way. These checks are pinned by
`group_sums_reproduce_the_totals_the_source_states` in
`src/activity/primary_helium.rs`. Because the maintainer entered the values by
hand, they have been human-read once. No second person has checked them.

### Processing

- **Xe-131m normalisation.** The PDF text layer, and therefore the kovan
  record, has a stray space in Xe-131m's value: `9. 3×10^6` (kovan) /
  `9. 3×106` (`pdftotext`). The value is 9.3×10^6 Bq, and it is written
  `9.3E+06` in the CSV. The noble-gas total above is consistent with that
  reading (a 9.3e6 entry is 0.07 % of the sum, so the total alone cannot
  confirm it; the reading rests on the space being the only oddity, with no
  digit missing, and on every other entry having the form `d.d×10^n`. The
  rendered page was not inspected for this entry).
- Notation: the source's `5.4×10^8` (flattened to `5.4×108` in the text layer,
  `5.4e8` in kovan) is written `5.4E+08`, matching the other CSVs.
  Significant figures are as published (two).
- Row order is the source's, which lists Kr-85 before Kr-85m, Xe-133 before
  Xe-133m and Xe-135 before Xe-135m, the reverse of Table 5's order for those
  pairs.
- The two-block layout was flattened to one row per nuclide. No unit
  conversion.

### Coolant activity is not a release

This is what the source calculates is circulating in the primary circuit, not
what leaves it. The activity it calculates is deposited on circuit surfaces
(its Table 4) is a separate quantity and is not digitised here.
`RESPONSIBLE_USE.md` applies in full: this is not a coolant activity for
HTR-10 or any other plant for any operational, licensing or safety purpose.

## HTR-10 individual effective dose versus distance, normal operation

`reference/htr10_normal_operation_individual_dose_by_distance.csv`, exposed by
`changi::activity::published_dose_by_distance`. Added 2026-09-28. **Reference
data only: nothing in this crate or in `htgr_sim_v1` consumes it, and `changi`
computes no dose.**

| Field | Value |
|---|---|
| Source | Liu Yuanzhong and Cao Jianzhu, "Fission product release and its environment impact for normal reactor operations and for relevant accidents" (citekey `yuanzhong2002fission`) |
| Publication | *Nuclear Engineering and Design* **218** (2002) 81–90, Elsevier |
| Table | Table 7, "Individual effective doses (mSv a⁻¹) to the public at various distance (km) from the release point in the azimuth where the maximum dose occurs". Journal p. 87 (PDF page 7), printed sideways |
| Quantity | Individual effective dose to a member of the public, **mSv per year** (column `effective_dose_msv_per_year`), at 12 distances from the release point, 0.5–75 km (column `distance_km`), along the azimuth of maximum dose only |
| Nature | **A published model result, not a measurement.** Calculated by the authors with US EPA's AIRDOS-EPA (Moore et al., 1979), "modified partly" by them |
| Basis (paper Section 3.2, pp. 85–86) | Airborne release from HTR-10 **normal operation**, i.e. the unfiltered, conservative annual release of Table 5. Released from a 40 m exhaust stack (the building is 12 m high) at 9 m/s. Pathways: gamma submersion, gamma from contaminated ground, inhalation, ingestion. Receptor: **adults** (food consumption in the source's Table 6, not reproduced). Meteorology: measurements from an observatory 7.5 km from the site. Dose factors: USDOE/EH-0070 (1988) and IAEA (1996) |
| What the paper says about it | The maximum is 1.4×10⁻⁴ mSv/a, about four orders of magnitude below the 1 mSv/a limit of Chinese standard GB8703. The text does not say where the maximum is. The table puts it at 1.5 km |
| Copyright | "© 2002 Elsevier Science B.V. All rights reserved." |
| Access terms | **Restricted.** No reuse licence is stated. The PDF is held in the maintainer's private literature repository and is **not** redistributed. Only the cited table of 12 values is reproduced, as ordinary scientific citation. No prose from the paper is copied |
| Date accessed / digitised | 2026-09-28 |

### How the values were obtained and verified

The maintainer digitised the table on 2026-09-28 (02:50 UTC) with kovan's
table digitiser, in the GUI table-grid mode on the rotated page. The kovan
record (`id = "table-7"`, `kind = "digitised_table"`, page 7) says every value
was entered by hand. **Unlike the Table 5 record (see the section above), this
one is the table it says it is.** Its 12 distances and 12 doses match Table 7.

An AI agent (Claude, under maintainer direction) checked all 24 numbers on
2026-09-28 against two other readings of the page:

1. **The PDF text layer.** In `pdftotext -f 7 -l 7 -layout` each distance sits
   directly above its dose, so every pair can be read unambiguously. The
   `-raw` mode puts the distances in a jumbled order and cannot be used to
   pair them. Its doses come out in the same order as `-layout`, 75 km back
   to 0.5 km.
2. **The rendered page**, rasterised locally, rotated and read by eye by the
   agent. This is not a human check.

All three readings agree on all 12 rows. **No discrepancy and no correction.**
The published maximum (1.4E−4 at 1.5 km) also matches the maximum the text
states. Because the maintainer entered the values by hand from the rendered
table, they have been human-read once. No second person has checked them.

### Processing

- Notation: the source prints `1.1E−4` with a Unicode minus (U+2212). The CSV
  uses an ASCII minus and the `1.1E-04` form of the other two CSVs.
  Significant figures are as published (two).
- The source's one-row-of-distances, one-row-of-doses layout was transposed to
  one row per distance, in the source's order (increasing distance).
- No unit conversion. The dose stays in mSv/a because `uom` 0.38 has no
  sievert quantity. The loader stores it as a plain `f64` and says so in the
  field name.

### What the curve does (recorded, not imposed)

The dose rises from 1.1E−4 at 0.5 km to 1.4E−4 at 1.5 km. It then falls at
every later distance, reaching 7.7E−6 at 75 km, 18 times below the peak. A
peak away from the stack is what an elevated release gives, but the paper does
not discuss the shape. Pinned by the tests in
`src/activity/published_dose_by_distance.rs`.

### This is not an accident dose, and not a basis for anything

The same paper's accident doses (Table 9) are a different quantity ~~and are not
digitised here~~ (**CORRECTED 2026-09-28**: now stored separately; see the next
section). The table uses real site meteorology and gives only the
worst direction, so it describes one published calculation and nothing more
general. `RESPONSIBLE_USE.md` applies in full. This is not a dose to the
public from HTR-10 or any other plant for any operational, licensing, siting,
emergency-planning or safety purpose. Storing it does not bring dose
assessment into `changi`'s current scope (see `crates/changi/CLAUDE.md`).

## HTR-10 individual accident dose versus distance (two design-basis accidents)

`reference/htr10_accident_individual_dose_by_distance.csv`, exposed by
`changi::activity::published_accident_dose_by_distance`. Added 2026-09-28.
**Reference data only: nothing in this crate or in `htgr_sim_v1` consumes it,
and `changi` computes no dose.** Parked in `changi` next to Table 7 at the
maintainer's direction (2026-09-28), pending their decision on where dose data
lives; the placeholder crate `buangkok` is the named future home, and nothing
has been added there.

| Field | Value |
|---|---|
| Source | Liu Yuanzhong and Cao Jianzhu, "Fission product release and its environment impact for normal reactor operations and for relevant accidents" (citekey `yuanzhong2002fission`) |
| Publication | *Nuclear Engineering and Design* **218** (2002) 81–90, Elsevier |
| Table | Table 9, "Individual doses caused by accidents of the HTR-10 (mSv)". Journal p. 88 (PDF page 8), printed with Tables 8 and 10 |
| Quantity | Individual dose to a member of the public, **mSv per accident** (no time basis or integration period is stated), at 13 distances from the release point, 0.25–75 km (column `distance_km`). Two quantities, **thyroid** and **whole-body**, for each of two accidents: columns `depressurization_thyroid_msv`, `depressurization_whole_body_msv`, `water_ingress_thyroid_msv`, `water_ingress_whole_body_msv` |
| Scenarios (paper Section 4.1) | The two design-basis accidents the paper identifies as giving the largest potential public dose. **Depressurization:** loss of primary helium through a ruptured 65 mm fuel-element charging tube; release of the primary-helium activity, plus desorption from circuit surfaces, dust-bound activity and helium-purification-system activity; no release from the coated particles (peak fuel temperature 1033 °C per the paper's cited transient analysis, against a 1600 °C limit). **Water ingress:** two-ended rupture of two steam-generator tubes with steam relief failed (at most 129.9 kg of water); ~23 % of the primary-helium activity, wash-off of steam-generator deposits, activity in ≤ 4.88 kg of corroded graphite. Both released via the 40 m stack with no filtering or plate-out credited. The releases are the paper's Table 8, ~~**not** digitised in this workspace~~ **CORRECTED 2026-09-28**: now stored, see the Table 8 section below |
| Nature | **A published model result, not a measurement.** Calculated by the authors with the German code STOERNEU |
| Basis stated (paper Section 4.2, pp. 88–89) | Pathways: gamma and beta submersion, gamma from contaminated ground, inhalation, ingestion. Stack height 40 m; reactor building 28 m high, 30 m wide |
| Basis **not** stated | Dose integration period; receptor age group; meteorology/dispersion conditions; azimuth (unlike Table 7, no "direction of maximum dose" is claimed); dose coefficients; whether "whole-body" is an effective dose. None of these is assumed in the loader or its docs |
| What the paper says about it | Compared with its Table 10 (emergency intervention levels of Chinese Nuclear Safety Criterion HAD 002/03; lowest sheltering levels 5 mSv whole-body and 50 mSv for lung, thyroid and other important organs), the doses are much lower than the lowest sheltering level, so the paper concludes no intervention would be needed even for the worst accident it analysed. Checked against the stored numbers (see below) |
| Copyright | "© 2002 Elsevier Science B.V. All rights reserved." |
| Access terms | **Restricted.** No reuse licence is stated. The PDF is held in the maintainer's private literature repository and is **not** redistributed. Only the cited table of 65 numbers (52 doses and 13 distances) is reproduced, as ordinary scientific citation. No prose from the paper is copied |
| Date accessed / digitised | 2026-09-28 |

### How the values were obtained and verified

The maintainer digitised the table on 2026-09-28 with kovan's table digitiser
(GUI table grid, every value entered by hand). The kovan record is
`id = "table-9"`, `kind = "digitised_table"`, `[source] page = 8`,
`[extraction] method = "pdf_native"`, saved 2026-09-28T03:15:53Z, in the
maintainer's notes for `yuanzhong2002fission`. It holds the table as one
series with columns distance, depressurisation thyroid, depressurisation whole
body, water ingress thyroid, water ingress whole body.

An AI agent (Claude, under maintainer direction) checked all 65 numbers on
2026-09-28 against two other readings of the page:

1. **The PDF text layer**, `pdftotext -f 8 -l 8 -layout`, in which each row
   of Table 9 comes out on one line in column order. Compared programmatically
   with the CSV: identical on all 13 rows. The `-raw` mode interleaves the
   columns (and the distances) in a scrambled order and **cannot** be used to
   assign values to columns.
2. **The rendered page**, rasterised locally and read by eye by the agent,
   mainly to confirm the column headers (Thyroid / Whole-body under each
   accident) and their order. This is not a human check.

All three readings agree on all 13 rows and all 52 doses. The kovan record
was also compared value by value with the CSV (after normalising its Unicode
minus): identical. **No discrepancy and no correction.** Because the maintainer
entered the values by hand from the rendered table, they have been human-read
once. No second person has checked them.

### Processing

- Notation: the source prints `1.7E−1` with a Unicode minus (U+2212), and one
  entry as plain `1.1` (water ingress thyroid at 0.25 km). The CSV uses ASCII
  minus and the `1.7E-01` / `1.1E+00` form of the other CSVs. Significant
  figures are as published (two).
- Layout and row order are the source's (increasing distance); column names
  were made explicit. No unit conversion. Doses stay in mSv as plain `f64`
  because `uom` 0.38 has no sievert quantity.
- The distance grid differs from Table 7's: it adds 0.25 and 0.75 km and has
  no 0.5 km row.

### What the numbers do (recorded, not imposed)

Pinned by the tests in `src/activity/published_accident_dose_by_distance.rs`
(2026-09-28):

- All four columns are largest at 0.25 km, the nearest tabulated distance,
  and fall strictly at every later distance. (Table 7, a different code and
  release, peaks at 1.5 km.)
- At every distance water ingress exceeds depressurization in both
  quantities, and thyroid exceeds whole-body in both accidents. The water
  ingress / depressurization thyroid ratio is 6.5 at 0.25 km, peaks at 7.1 at
  1.5 km, and falls to 3.3 at 75 km.
- Largest values: whole-body 0.20 mSv and thyroid 1.1 mSv, both water
  ingress at 0.25 km. Against the paper's comparison levels these are 25×
  below 5 mSv and about 45× below 50 mSv, so the paper's statement holds for
  the table as printed. That reproduces the paper's argument; it is not a
  finding of this workspace.

### Not a basis for anything

The table describes one published calculation on an unstated meteorological
basis, for two design-basis accidents only, with no coated-particle release.
`RESPONSIBLE_USE.md` applies in full. This is not a dose to the public from
HTR-10 or any other plant for any operational, licensing, siting,
emergency-planning (including emergency-zone sizing) or safety purpose; this
workspace uses it for research-grade safety analysis only. Storing it does not
bring dose assessment into `changi`'s current scope (see
`crates/changi/CLAUDE.md`).

## HTR-10 airborne release for two design-basis accidents

`reference/htr10_accident_airborne_release.csv`, exposed by
`changi::activity::accident_airborne_release`. Added 2026-09-28. **Reference
data only: nothing in this crate or in `htgr_sim_v1` consumes it.** It is the
release behind the Table 9 doses in the section above, and reuses that
loader's `AccidentCase` enum.

| Field | Value |
|---|---|
| Source | Liu Yuanzhong and Cao Jianzhu, "Fission product release and its environment impact for normal reactor operations and for relevant accidents" (citekey `yuanzhong2002fission`) |
| Publication | *Nuclear Engineering and Design* **218** (2002) 81–90, Elsevier |
| Table | Table 8, "The HTR-10 accidental radioactivity release (Bq)". Journal p. 88 (PDF page 8), printed with Tables 9 and 10 |
| Quantity | Activity released to the environment per nuclide, **Bq per accident** (unit in the table title; no release duration or time profile is stated). Columns `depressurization_release_bq`, `water_ingress_release_bq` |
| Scenarios (paper Section 4.1) | As in the Table 9 section above. **Depressurization** (ruptured 65 mm fuel-element charging tube): primary-helium activity, desorption from circuit surfaces, 10 % of the dust, and helium-purification-system activity (100 % of noble gases, H-3 and C-14; 10 % of iodine and metal fission products). **Water ingress** (two steam-generator tubes ruptured, steam relief failed): ~23 % of the primary-helium activity, water wash-off of the whole steam-generator deposit, activity in ≤ 4.88 kg of corroded graphite. No coated-particle release; stack release with no filtering or plate-out credited |
| Contents | 18 nuclides: 8 noble gases (Kr-83m, Kr-85m, Kr-85, Kr-88, Xe-131m, Xe-133m, Xe-133, Xe-135), 4 iodines (no I-134), Sr-90 (not Sr-89 as in Tables 3 and 5), Cs-134, Cs-137, Ag-110m, H-3, C-14 |
| Nature | **A published model result, not a measurement.** The paper names no code for the release calculation; STOERNEU is the code it names for the doses (Table 9) computed from it |
| Basis **not** stated | Release duration/time profile; the inventory state (e.g. Table 3's end-of-life primary helium) the release starts from |
| Copyright | "© 2002 Elsevier Science B.V. All rights reserved." |
| Access terms | **Restricted.** No reuse licence is stated. The PDF is held in the maintainer's private literature repository and is **not** redistributed. Only the cited table of 36 values is reproduced, as ordinary scientific citation. No prose from the paper is copied |
| Date accessed / digitised | 2026-09-28 |

### How the values were obtained and verified

The maintainer digitised the table on 2026-09-28 with kovan's table digitiser
(GUI table grid, every value entered by hand). The kovan record is
`id = "table-8"`, `kind = "digitised_table"`, `[source] page = 8`,
`[extraction] method = "pdf_native"`, saved 2026-09-28T03:34:03Z, in the
maintainer's notes for `yuanzhong2002fission`.

An AI agent (Claude, under maintainer direction) checked all 36 values on
2026-09-28 against two other readings of the page:

1. **The PDF text layer**, `pdftotext -f 8 -l 8 -layout`, in which each row
   of Table 8 comes out on one line in column order. Compared
   programmatically with the CSV and with the kovan record: identical on all
   18 rows, all 36 values and the order.
2. **The rendered page**, rasterised locally and read by eye by the agent
   (not a human check), to confirm the column headers and the last row's
   label.

**No value discrepancy.** One label correction, below.

### Correction: the paper's "C-4" is stored as C-14

The last row is printed **"C-4"**: in the text layer, on the rendered page,
and in the kovan record. No nuclide C-4 exists. It is stored as **C-14**
because (a) the paper's Section 4.1.1.4 names C-14 among the species
released from the helium purification system; (b) its Table 5 lists C-14 in
the same position, after H-3; (c) the water-ingress value, 1.9×10^4 Bq, is
0.30 of the primary-helium C-14 total the paper's text states (6.3×10^4 Bq),
the same fraction as every noble gas and H-3 (below). The loader returns
`None` for `"C-4"`. If the maintainer prefers the printed label, change the
CSV and the two tests that name it.

### Processing

- Notation: the source's `6.3E8` is written `6.3E+08`, matching the other
  CSVs. Significant figures are as published (two).
- Row order and column order are the source's. No unit conversion.

### What the numbers do (recorded, not imposed)

Pinned by the tests in `src/activity/accident_airborne_release.rs`
(2026-09-28):

- **The paper's "approximate 23 %" does not reproduce.** For water ingress,
  Table 8 divided by the Table 3 primary-helium activity is 0.295–0.315 for
  all eight noble gases and H-3, and 0.30 for C-14 (against the text's
  6.3×10^4 Bq). The fraction is uniform across half-lives from hours to
  10.7 years, so a single factor of about 0.30 appears to have been applied;
  0.23 lies outside two-significant-figure rounding of every ratio. The paper
  does not explain the difference (a different helium inventory from Table
  3's end-of-life one, or a different fraction, are both possible). Reported
  as a disagreement in the source, not reconciled.
- For depressurization, every noble gas and H-3 exceeds its Table 3
  primary-helium activity (1.17× for Kr-83m to 50× for Kr-85), consistent
  with the helium-purification-system inventory being added.
- Totals: 4.51×10^10 Bq (depressurization, dominated by Xe-133 and H-3) and
  5.67×10^9 Bq (water ingress). Depressurization releases more of every
  noble gas, H-3 and C-14; water ingress releases more I-131 (8.8×), I-133,
  Sr-90, Cs-134 and Cs-137 (2.4×). The paper states no totals for this table,
  so there is nothing to check them against.
- The direction matches Table 9, where water ingress gives the larger thyroid
  and whole-body doses, but the paper gives no quantitative link that could
  be checked, and none is claimed.

### Not a basis for anything

One published calculation for two design-basis accidents with no
coated-particle release. `RESPONSIBLE_USE.md` applies in full: this is not a
release figure for HTR-10 or any other plant for any operational, licensing,
siting, emergency-planning or safety purpose; this workspace uses it for
research-grade safety analysis only.
