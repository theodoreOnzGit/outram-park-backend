# EBR-II reference data (transcribed numbers, with citations)

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety-critical decisions or any operational purpose (`RESPONSIBLE_USE.md`).
> EBR-II was shut down in 1994; these are published benchmark specifications
> and measurements, not operational data.

**Status: AI transcription, 2026-11-10, NOT yet reviewed by a human.** Every
number was copied as printed. Read the cited table and page before you rely on a
value, and record your review in this README when you do.

## Why these files may be here

The maintainer granted a data-policy exception on 2026-11-10 (`DATA_POLICY.md`,
"Maintainer exception: published EBR-II benchmark reports"). Later that day the
maintainer added: "Allow any citable information in scientific literature".
The exception covers **numbers only**, each with its document, table or figure,
and page. The PDFs, page images, figures and copied prose are **not** in this
repository; they stay in the maintainer's private literature repository.

## Sources

| Key | Document | Licence / access terms |
|---|---|---|
| A226 | L. Sumner, T. Y. C. Wei (2012), *Benchmark Specifications and Data Requirements for EBR-II Shutdown Heat Removal Tests SHRT-17 and SHRT-45R*, ANL-ARC-226 Rev. 1, Argonne National Laboratory | ANL contractor report; no unlimited-distribution statement on the copy read. Used under the maintainer's exception. |
| A228 | T. Fei, A. Mohamed, T. K. Kim (2013), *Neutronics Benchmark Specifications for EBR-II Shutdown Heat Removal Test SHRT-45R*, ANL-ARC-228 Rev. 1, Argonne National Laboratory | As A226. |
| W64 | R. Kirn, W. B. Loewenstein (1964), *EBR-II Wet Critical Experiments*, ANL-6864, Argonne National Laboratory | As A226. |
| T1819 | IAEA (2017), *Benchmark Analysis of EBR-II Shutdown Heat Removal Tests*, IAEA-TECDOC-1819 | IAEA copyright, with a reuse permission. No file here yet transcribes it. |
| C85 | Lentz, Buschman, Smith (1985), *EBR-II: Twenty Years of Operating Experience*, CONF-850722-1 | No file here yet transcribes it. |

URLs, DOIs and OSTI identifiers were **not re-checked** in this session and are
deliberately left out rather than guessed. Accessed 2026-11-10 from the
maintainer's private corpus.

## Files

Each CSV starts with `#` comment lines that name the document, the table, the
printed page (and PDF page where they differ), the units, and any reading
decision. A226 PDF page = printed page + 16; A228 PDF page = printed page.

| File | Source | What it holds |
|---|---|---|
| `anl6864_table5.csv` | W64 Table V, p.22 | 1963 approach-to-critical loading sequence: per-subassembly U-235 and U-238 mass (g) |
| `arc226_initial_conditions_text.csv` | A226 §§3.1.2-3.1.4, 3.2.2-3.2.4, 4.1.5-4.1.6 | Initial conditions and positions stated in running text |
| `arc226_pump_model.csv` | A226 Eqs 3.7a-3.7l p.16; Table 3.1.4.1.2 p.17 | Primary pump (NATDEMO) homologous coefficients and rated parameters |
| `arc226_table3_1_1_1_and_3_2_1_1.csv` | A226 Tables 3.1.1.1 p.9, 3.2.1.1 p.24 | SHRT-17 and SHRT-45R test descriptions |
| `arc226_table3_1_4_1_1.csv` | A226 Table 3.1.4.1.1 p.14 | SHRT-17 primary pump speeds (boundary condition) |
| `arc226_table3_1_4_2_1_power.csv` | A226 Table 3.1.4.2.1 p.19 | SHRT-17 fission, decay-heat and total power |
| `arc226_table3_1_4_2_1_ihx_flow.csv` | A226 Table 3.1.4.2.1 (number reused) p.21 | SHRT-17 intermediate IHX inlet flow |
| `arc226_table3_1_4_2_2.csv` | A226 Table 3.1.4.2.2 p.21 | SHRT-17 intermediate IHX inlet temperature |
| `arc226_table3_2_4_1_1.csv` | A226 Table 3.2.4.1.1 p.29 | SHRT-45R primary pump speeds |
| `arc226_table3_2_4_1_2.csv` | A226 Table 3.2.4.1.2 p.31 | SHRT-45R auxiliary EM pump current, voltages, resistance |
| `arc226_table3_2_4_2_1.csv` | A226 Table 3.2.4.2.1 p.36 | SHRT-45R intermediate IHX inlet flow |
| `arc226_table3_2_4_2_2.csv` | A226 Table 3.2.4.2.2 p.37 | SHRT-45R intermediate IHX inlet temperature |
| `arc228_table3_1_1.csv` | A228 Table 3.1.1 p.17 | Subassembly element design parameters at 20 C |
| `arc228_table3_1_2.csv` | A228 Table 3.1.2 p.18 | Subassembly structure design parameters at 20 C |
| `arc228_table3_2_1.csv` | A228 Table 3.2.1 p.21 | Assembly 1A1 isotopic number densities, **depleted** (start of SHRT-45R), not fresh |
| `arc228_table3_2_2.csv` | A228 Table 3.2.2 p.21 | U-5Fs alloy elemental composition (wt %) |
| `arc228_table3_3.csv` | A228 Table 3.3 pp.22-24 | Run 138B design and operating data |
| `arc228_text_values.csv` | A228 pp.8, 11, 19, 20, 25 | Scalar values from running text |

## Processing, assumptions and known limitations

- **Reading decisions** are recorded in each file's header. Example: W64's scan
  prints a space for the decimal point ("1889 90"). It is read as a decimal
  point, and that reading is confirmed by the Table III cumulative U-235
  totals.
- **The sources disagree with each other** in several places. Each file keeps
  the value its own source prints:
  - control-rod stroke: 43.815 vs 35.56 cm;
  - rod positions: 21.06 vs 21.29 cm;
  - cladding wall thickness: 0.305 vs 0.315 mm;
  - the third branch of the U-5Fs expansion correlation: (T-913) in A226 vs
    (T-273) in T1819;
  - SHRT-45R initial intermediate flow: 5079.5 vs 5474.5 gpm. A226 prints the
    calibration temperature as "XX F".
- **A fresh core is not the SHRT core.** Every SHRT core-physics number is for
  the burned Run 129C / 138B core. The only measured fresh-core reference is
  the 1963 wet critical (W64). That core was 48.4 % enriched, not the later
  67 %. Its minimum critical mass was 181.2 kg U-235 at 600 F (W64 p.27), and
  W64 does not state an uncertainty for it.
- **Not transcribed yet:**
  - W64's reference loading map (Fig. 16) and its rod calibration curves;
  - the per-subassembly power and flow maps (A226 figures);
  - the measured transient traces (T1819).

  These are plot-only data. Digitising them must be recorded under "Data
  Provenance" in `DATA_POLICY.md`.
- **Not in any of these documents:**
  - A228 Appendix B/C, the depleted-composition CSVs, which were distributed
    separately;
  - steel elemental compositions;
  - the U-235 fraction of the depleted uranium;
  - the B-10 enrichment of the half-worth control rods;
  - 1963 element and subassembly designs.

## Review record

| Date | Reviewer | Files | Result |
|---|---|---|---|
| (none yet) | | | |
