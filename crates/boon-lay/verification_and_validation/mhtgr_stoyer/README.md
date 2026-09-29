# MHTGR end-to-end workflow check against Stoyer et al. (GitHub #413)

**Research, education and V&V only.** This is library verification of the
`boon-lay` TRISO-ATOPS fork. It is not a source term for any plant.

## Source

B. Stoyer, A. Raichart, et al., *TRISO-ATOPS: A Mechanistic Source Term Model
for Gas-Cooled Reactors*, Nuclear Technology (2026), section III.

- **Access tier:** proprietary. The values the check needs are committed here
  as a **cited table**, with the table and page of every value. **The PDF is
  not committed.**
- **Pages:** `pdf_page` = printed page + 1.

## Files

| File | Paper | Pages (printed / pdf) | Content |
|---|---|---|---|
| `constants.csv` | Tables 2, 3, 12; s.III.B.1 | 7 / 8; 21 / 22; 19 / 20 | failure fractions and constants, Case A and Case B |
| `table06_case_a_inventory_ci.csv` | Table 6 | 14-15 / 15-16 | 350 MWth inventories [Ci] per radial section |
| `table07_core_temperature_k.csv` | Table 7 | 15 / 16 | 14 x 3 core temperatures [K] |
| `table09_case_a_normal_operation_ci.csv` | Table 9 | 17-18 / 18-19 | Case A graphite / circulating / plate-out / HPS [Ci] |
| `table10_case_a_accident_release_ci.csv` | Table 10 | 18 / 19 | Case A initial and final releases [Ci], final **as printed** (the paper's x10 building reduction already applied) |
| `table11_case_b_inventory_ci.csv` | Table 11 | 20-21 / 21-22 | 600 MWth inventories |
| `table13_case_b_normal_operation_ci.csv` | Table 13 | 22-23 / 23-24 | Case B pools |
| `table14_case_b_accident_release_ci.csv` | Table 14 | 23 / 24 | Case B releases, final as printed |
| `upstream_case_{a,b}.csv` | computed | -- | upstream TRISO-ATOPS at de374c8 on these inputs, unrounded |
| `upstream_case_{a,b}_diagnostic_kplate_7p5e-4.csv` | computed | -- | the same with k_plate = 7.5e-4 1/s (diagnostic, see below) |

## Extraction

The tables were extracted with `pdftotext -raw` (poppler), one page at a time,
and parsed by a deterministic regex: a mass-number line followed by a line of
symbol and values. **No table value was typed by hand**, except
`constants.csv`, which was transcribed from the same text layer. Rows were
spot-checked against the rendered pages 16 and 24. Row counts are 64 per
inventory/pool table and 46 per release table.

## Reproduce

```bash
python3 crates/boon-lay/dev/mhtgr_stoyer_upstream.py   # upstream, needs numpy
cargo test --release -p boon-lay --test mhtgr_stoyer_workflow -- --nocapture
```

## Findings (2026-09-29)

**Port vs upstream.** The worst relative difference is **5.6e-12** across every
pool of every nuclide, in both cases.

**Upstream (and the port) vs the paper, with Table 3 as printed.** Here
`k_plate = 7.50E-05 /s`.
- Graphite agrees for 40/40 nuclides.
- Only 11/64 circulating values agree: long-lived metals run ~10x high.
- Only 10/24 HPS values agree: halogens run ~5x high.

**With `k_plate = 7.5e-4 /s`.** This is upstream's own GUI and manual default.
Every printed value is reproduced within 2 %, in both cases:
- graphite 40/40;
- circulating 64/64;
- plate-out 52/52;
- HPS 24/24;
- initial release 46/46.

On this evidence, Table 3's `7.50E-05` is a misprint for `7.50E-04`. The
committed inputs keep the value as printed, and the 7.5e-4 run is labelled a
diagnostic.

## Fig. 5 and the final (heat-up) releases (added 2026-09-29)

**Source of the curves.** `fig05_accident_temperature_c.csv` holds the
maintainer's (teddy0) **manual digitisation** in kovan.
- Kovan artifact id: `fig-5-transient-temperature-profiles-for-the-mhtgr-test-cases-5-curve-from-ref-15`
  (kind `digitised_graph`).
- Hand-placed points, digitised 2026-09-29T12:04:42Z, from pdf page 16.
- Calibration: x, px 100.303 = 0 h to px 509.756 = 140 h; y, px 341.680 =
  200 °C to px 30.210 = 1600 °C, both linear.
- Stored in the maintainer's kovan notes, outside this repo.
- The values are copied verbatim.
- **Negative times:** two points sit at slightly negative time (20 % curve
  -0.36 h; 25 % curve -0.54 h). This is digitisation noise. They are kept
  in the CSV as digitised, and both drivers clamp them to t = 0. None is
  dropped.

**The paper's "-200 / -400 / -600 °C" rule, over the whole transient.**
Each curve was compared with the 5 % curve lowered by the stated offset,
interpolated at the curve's own points:
- **From ~10 h on:** agreement within -12 to +15 °C.
- **In the first ~10 h:** the curves are colder than the rule, by up to
  131 °C (20 %), 28 °C (25 %) and 81 °C (50 %).
- **Peaks:** 1565 / 1361 / 1154 / 963 °C, against 1565 / 1365 / 1165 / 965.

The rule describes the plotted peaks, not the plotted start of the
transient. The **digitised curves are used as plotted**, because they are
what the paper's code was run on.

**Method.**
- Each curve is applied uniformly to all 14 x 3 nodes.
- There is one `accident_case` run per curve, through upstream and through
  the port.
- The last totals are combined by Eq. (29), weights 0.05 / 0.2 / 0.25 / 0.5.
- The paper's post-processing is then replicated and **flagged**: final =
  initial + (Eq. 29 - initial) / 10. This is "reduced by an order of
  magnitude ... applied to releases following the initial breach",
  s.III.A.5.
- `upstream_case_*_accident.csv` holds every intermediate. It also carries
  the alternative reading, Eq. 29 / 10, which differs by < 1 %.

**Results.**
- **Port vs upstream:** worst relative difference **2.2e-11**.
- **Against the paper's final column:** a near-uniform **~0.83**.
  - Case A: median 0.830, range 0.80-0.93.
  - Case B: median 0.83. Outliers are Ag-111 (1.75 at printed `k_plate`,
    0.71 at 7.5e-4) and Cs-134 (~0.64).
  - The plate-out constant barely moves the final releases.
- **Leading hypothesis for the common factor:** the **venting (breathing)
  fraction**. It multiplies every fuel and graphite release and depends on
  each curve's starting temperature.
  - The rendered figure shows t = 0 markers on the 5 % curve (~800 °C) and
    the 50 % curve (~200 °C) that are **not in the digitisation**. Their
    first digitised points are 923 °C at 0.32 h and 326 °C at 1.35 h.
  - Some early points are non-monotonic, which makes upstream's venting
    mask gappy.
  - The resulting vent fractions are 0.44 / 0.48 / 0.62 / 0.41 (5 / 20 /
    25 / 50 %).
  - Not corrected here, because adding points would be a second, unreviewed
    digitisation. Re-digitising the first few markers of the 5 % and 50 %
    curves was requested on #413.
