# EPA FGR-15 — adult effective dose-rate coefficients, air submersion and ground surface

Adult effective dose-rate coefficients for **external** exposure, for the 23
nuclides in Liu and Cao (2002) Tables 5 and 8 (HTR-10 airborne releases), plus
**Ba-137m**, the gamma-emitting daughter of Cs-137. They
are extracted from **Federal Guidance Report No. 15** so that `buangkok` can
compute the submersion and ground-shine pathways (gh:#379).

The data file is [`epa-fgr15-adult-external-coefficients.csv`](epa-fgr15-adult-external-coefficients.csv),
beside this record. Its columns are `nuclide`,
`air_submersion_sv_m3_per_bq_s` (Sv m^3 Bq^-1 s^-1, Table 4-6) and
`ground_surface_sv_m2_per_bq_s` (Sv m^2 Bq^-1 s^-1, Table 4-1).

---

## Provenance

| Field | Value |
|---|---|
| Source | Bellamy, M.B., Samuels, C.E., Dewji, S.A., Leggett, R.W., Hiller, M., Veinot, K., Manger, R.P., Ryman, J.C., Easterly, C.E., Hertel, N.E., Stewart, D.J., Eckerman, K.F. *External Exposure to Radionuclides in Air, Water and Soil.* Federal Guidance Report No. 15, **EPA 402-R-25-001, revised July 2025**. Oak Ridge National Laboratory for the Office of Radiation and Indoor Air, U.S. EPA. |
| URL | <https://www.epa.gov/system/files/documents/2025-07/fgr15_rev2025july_final_508.pdf> |
| Catalogued copy | Kovan standard open corpus, `kovan-standard-open-corpus/epa/fgr-15-epa-402-r-25-001.pdf` in the `reactor-literature` submodule (`crates/kovan-literature/reactor-literature/`, commit `686c94bf`) |
| Access tier | **Open**, on EPA's "Copyright Status" statement: free for non-commercial, scientific and educational use. The basis is recorded in the corpus README. This is the same basis as FGR-11 and FGR-13. |
| Edition | The **July 2025 revision**. EPA withdrew the earlier versions (402-R-18-001, 402-R-19-002) because of errors in the coefficient tables; do not substitute them. |
| Tables used | **Table 4-6**, *Reference person effective dose rate coefficients for air submersion*; **Table 4-1**, *… for ground surface*. **Adult** column only. |
| Date extracted | 2026-09-29 |

## Processing

1. `kovan-cli lit import <pdf> --markdown-out <scratch>.md`: the kovan PDF
   pipeline, run on the corpus PDF. It produced 998 283 characters. The table
   rows come out as text such as `Te- 131m 1.19E - 15 … 9.08E - 16`: nuclide,
   then six ages from Newborn to Adult.
2. A deterministic regex over that Markdown, restricted to each table's line
   range, matched `<Symbol> - <mass>[m]` followed by six numbers. The
   whitespace inside `E - 15` was removed, and the **sixth (Adult)** value
   kept. Each table parsed to **1 246 nuclides**, and each of the 24 target
   nuclides matched **exactly once** per table. The script asserted this.
   Ba-137m was appended in a second run of the same parse.
3. No value was typed by hand. No rounding: values are the source's 3
   significant figures.

## Checks and caveats

- **Spot check against the older FGR-12 (1993) values, by recollection and not
  a catalogued comparison:** Xe-133 submersion 1.22e-15 against ~1.6e-15, and
  Kr-88 9.73e-14 against ~1.0e-13. These are the expected size of an
  edition-to-edition change (ICRP 103 weighting, new phantoms). This is not a
  verification of the extraction.
- **Progeny are separate rows in FGR-15.** Cs-137's own coefficient excludes
  Ba-137m, the daughter that emits its 662 keV gamma. So does any
  parent-daughter pair, e.g. Kr-88 → Rb-88. A caller that wants chains must
  add the daughters. **This matters:** Ba-137m's ground coefficient
  (3.87e-16) is 129x Cs-137's own (3.01e-18), and for the Liu and Cao
  accident releases, adding it raised the external dose ~15x
  (`crates/buangkok/tests/liu_cao_external_dose_cross_check.rs`, which adds
  it with the ENDF/B-VIII.0 branching 0.94699 via boon-lay). Other daughters
  (e.g. Rb-88) are still omitted there.
- **Adult, effective dose only.** No organ (thyroid) coefficients: FGR-15 is
  external exposure. Inhalation coefficients are in FGR-11, whose 1988 tables
  did **not** extract through kovan (scanned image; the body text came out,
  the tables did not). That is filed against kovan (gh:#390), not worked around.
