# References — `buangkok`

Provenance for every external number this crate holds, per the workspace
`CLAUDE.md` "Responsible use & data policy" rule: source, organisation, title,
access terms, URL, date accessed, and any processing or assumptions applied.

The two sections below were written when the tables lived in `changi` and
moved here with them on 2026-09-28 (maintainer: "move table 7 and 9 to
buangkok"); paths are updated, the history is otherwise unchanged. The
accident *release* table (Liu and Cao 2002, Table 8) stays in
`crates/changi/docs/References.md`, and `AccidentCase` is defined in changi.

Ported pyDOSEIA code provenance (MIT) lives in `NOTICE` and the pyDOSEIA
docs, not here.

---

## HTR-10 individual effective dose versus distance, normal operation

`reference/htr10_normal_operation_individual_dose_by_distance.csv`, exposed by
`buangkok::published::normal_operation_dose_by_distance`. Added 2026-09-28. **Reference
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
`src/published/normal_operation_dose_by_distance.rs`.

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
`buangkok::published::accident_dose_by_distance`. Added 2026-09-28.
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

Pinned by the tests in `src/published/accident_dose_by_distance.rs`
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
