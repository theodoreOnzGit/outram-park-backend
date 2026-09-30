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
  (kind `digitised_graph`), from pdf page 16.
- First digitised 2026-09-29T12:04:42Z.
- **Re-digitised 2026-09-30T01:50:02Z** (kovan commit 066e691): t = 0 markers
  added to the 5 % and 50 % curves, and four erroneous 50 % points removed.
- Calibration: x, px 100.303 = 0 h to px 509.756 = 140 h; y, px 341.680 =
  200 °C to px 30.210 = 1600 °C, both linear.
- Stored in the maintainer's kovan notes, outside this repo. Values copied
  verbatim.
- **Negative times:** four points sit at slightly negative time (5 % curve
  -0.12 h; 20 % -0.36 h; 25 % -0.54 h; 50 % -0.36 h). This is digitisation
  noise. They are kept in the CSV as digitised, and both drivers clamp them
  to t = 0.

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

**REPORTED RESULT (2026-09-30, #413 check (b)): dense PCHIP resampling of the digitised curves.**

**The rule, fixed and posted on #413 before running.** The reported result
uses a dense, shape-preserving resampling of the maintainer's digitised
points, whatever it gives.
- Reason: upstream's `coolant_release` integrates dn/dt by cumulative
  trapezoid over the supplied time points, so its vent fraction depends on
  the grid. The paper fed a dense profile.
- Processing: `dev/mhtgr_stoyer_fig5_pchip.py`. It clamps t < 0 to 0 and
  resamples with PCHIP (Fritsch-Carlson, scipy's `PchipInterpolator` rule,
  in numpy) every 0.1 h. Output: `fig05_accident_temperature_c_pchip_0p1h.csv`.
  These are **interpolated values, not digitised points**.
- Upstream outputs: `upstream_case_*_accident_pchip.csv`.
- Port vs upstream on the dense grid: see the test.

**Vent fractions: sparse / dense / exact ideal-gas 1 − T₀/T_peak.**

| Curve | Sparse | Dense | Exact |
|---|---|---|---|
| 5 % | 0.551 | 0.416 | 0.423 |
| 20 % | 0.478 | 0.465 | 0.469 |
| 25 % | 0.616 | 0.534 | 0.539 |
| 50 % | 0.519 | 0.610 | 0.618 |

The dense values reach the exact values within 1.6 %, as predicted. The 20 %
curve keeps its one digitised down-step near t = 0 (595.0 → 592.8 °C). The
50 % curve's flat digitised points near the peak (dT = 0) are still admitted
by upstream's `dT/dt >= 0` mask.

**Final / paper, median by group, sparse → dense.** Per nuclide:
`final_release_ratio_sparse_vs_pchip.csv`.

| Group | A, printed `k_plate` | B, printed `k_plate` | B, 7.5e-4 |
|---|---|---|---|
| volatiles (Kr, Xe, I, Te) | 0.983 → **0.884** | 0.986 → **0.887** | 0.982 → 0.884 |
| Cs | 0.862 → 0.858 | 0.843 → 0.840 | 0.758 → 0.754 |
| Sr / Ba / Eu | 0.931 → 0.843 | 0.946 → 0.866 | 0.936 → 0.849 |
| Ag | 1.134 → **0.874** | 1.386 → 1.352 | 0.834 → 0.799 |
| constant-D metals | 0.878 → **0.907** | 0.878 → **0.907** | 0.878 → 0.907 |
| **all nuclides** | 0.881 → **0.903** (0.84-0.94) | 0.881 → **0.904** (0.67-1.74) | 0.881 → 0.900 |

Case A at 7.5e-4 matches Case A at the printed `k_plate` to the third digit.

**Against the prediction.**
- The constant-D metals move toward 1 but do not reach it: 0.878 → 0.907,
  against a predicted ~0.91.
- The groups weighted to the hot curves drop by ~10 %, because the 5 % and
  20 % curves' vent fractions fall.

**Outcome.** The spread between groups narrows markedly. In Case A every
nuclide now lies within 0.84-0.94. A common factor of ~0.90 remains, and it
is not diagnosed. The Case B outliers remain: Cs-134 0.67, Kr-85 0.68 and
Ag-111 (1.74 at the printed value, 0.70 at 7.5e-4).

**Sparse-point results, kept for comparison (superseded as the reported
result).**

**Sparse-point result on the re-digitised curves (2026-09-30; superseded by the dense resampling above).**
- **Port vs upstream:** worst relative difference **3.9e-11**.
- **Final vs the paper:** median **0.88**, both cases and both `k_plate`.
  - Case A: range 0.86-1.15. The volatiles (I, Te, Xe) are at 0.97-0.99;
    the metals at 0.86-0.94; Ag-110m and Ag-111 at 1.12 and 1.15.
  - Case B: range 0.66-1.79 at the printed `k_plate`, 0.66-0.99 at 7.5e-4.
    The outliers are Cs-134 (0.67), Kr-85 (0.76) and Ag-111 (1.79 at the
    printed value).
  - Per nuclide, before and after: `final_release_ratio_before_after.csv`.
- **Vent fractions, before -> after:**

| Curve | Before | After |
|---|---|---|
| 5 % | 0.444 | **0.551** |
| 20 % | 0.478 | 0.478 |
| 25 % | 0.616 | 0.616 |
| 50 % | 0.407 | **0.519** |

- **The 50 % curve now rises monotonically to its peak**, so its dip no
  longer gaps the rising part of the vent mask. Points after the peak that
  step up again (noise in the decline) are still admitted by upstream's
  `dTdt >= 0` mask: 27 of 46 samples are kept.
- The 20 % curve still has one down-step before its peak, between -0.36 h
  (595.0 °C) and 0.66 h (592.8 °C). It is left as digitised.
- The remaining ~12 % shortfall is concentrated in the metals, and is not
  diagnosed.

Earlier result, on the 2026-09-29 digitisation (superseded): port vs upstream
2.2e-11; ~~median 0.83~~ (Case A 0.80-0.93).

**Earlier results (2026-09-29 digitisation, superseded).**
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

## DIAGNOSTIC (not adopted): temperature sensitivity of the final releases (#413 check (a), 2026-09-30)

**This section does not change the result.** The committed #413 result
stays on the Fig. 5 curves as digitised. This check only asks whether small
digitisation temperature errors could explain the residual.

**Method.**
- All four curves are shifted uniformly by -15 ... +15 °C.
- Only upstream TRISO-ATOPS is re-run, with the same method: Eq. 29, and the
  x10 applied after the initial puff. The port matches upstream to < 4e-11
  on the unshifted path and is not re-run.
- Printed `k_plate` at every shift; 7.5e-4 at 0 and ±15 °C.
- No other input varies. A uniform shift is a crude proxy, since real
  digitisation error varies along each curve.
- Driver: `dev/mhtgr_stoyer_tshift_diagnostic.py`. Data:
  `diagnostic_tshift_final_ratio.csv`.
- A prediction was posted on #413 before running.

**Median ratio to the paper, printed `k_plate`, by shift.**

| Group | Case | -15 | -10 | -5 | 0 | +5 | +10 | +15 |
|---|---|---|---|---|---|---|---|---|
| volatiles (Kr, Xe, I, Te) | A | 0.880 | 0.902 | 0.925 | 0.983 | 1.007 | 1.033 | 1.058 |
| Cs | A | 0.821 | 0.833 | 0.845 | 0.862 | 0.879 | 0.896 | 0.913 |
| Sr / Ba / Eu | A | 0.816 | 0.853 | 0.891 | 0.931 | 0.973 | 1.016 | 1.062 |
| Ag | A | 0.893 | 0.969 | 1.050 | 1.134 | 1.222 | 1.315 | 1.412 |
| constant-D metals (Zr, Nb, Mo, Ru, Ce, ...) | A | 0.884 | 0.882 | 0.880 | 0.878 | 0.876 | 0.874 | 0.872 |
| volatiles | B | 0.883 | 0.905 | 0.928 | 0.986 | 1.010 | 1.035 | 1.061 |
| Cs | B | 0.800 | 0.815 | 0.829 | 0.843 | 0.858 | 0.872 | 0.887 |
| Sr / Ba / Eu | B | 0.841 | 0.873 | 0.906 | 0.946 | 0.981 | 1.020 | 1.064 |
| Ag | B | 1.354 | 1.364 | 1.375 | 1.386 | 1.397 | 1.410 | 1.422 |
| constant-D metals | B | 0.884 | 0.882 | 0.880 | 0.878 | 0.876 | 0.874 | 0.871 |

- The vent fractions move by less than 1 % over ±15 °C.
- The 7.5e-4 diagnostic shows the same pattern. Per nuclide (including
  Kr-85, Cs-134, Cs-137, Sr-90, Ag-110m, Ag-111 and Ce-144) is in the CSV.

**Reading.**
- **The volatiles move 6-8 % per 15 °C**, more than the predicted 3-6 %. Part
  of that is a step of about 6 % between -5 and 0 °C, where points of the
  5 % curve cross upstream's 1500 °C switch in the Kr-diffusivity branch.
- **Cs moves 3-6 %, Sr 14 %, and Ag (Case A) about 25 % per 15 °C.**
- **The constant-D metals do not move** (-0.7 % per +15 °C, through the vent
  fraction). These are the bulk of the nuclides at 0.87-0.88. Upstream gives
  them D = 1e-19 whatever the temperature.

**Verdict.**
- **The hypothesis is not supported as the explanation for the
  metal/volatile split.** The largest group of low metals has no temperature
  dependence, so no temperature error can move it.
- For the temperature-sensitive groups there is no single uniform shift that
  reconciles them. Implied shifts to reach ratio 1:
  - volatiles ≈ +2 °C;
  - Sr/Ba/Eu ≈ +7 °C;
  - Cs > +15 °C (not reached in the range);
  - Ag (Case A) ≈ -8 °C, in the opposite direction.
- Temperature error of a few °C can explain the volatiles' last 2 %. It
  cannot explain the ~12 % shortfall of the constant-D metals, which remains
  undiagnosed.

