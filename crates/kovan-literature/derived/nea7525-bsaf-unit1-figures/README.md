# Fukushima Daiichi unit 1 curves, digitised from BSAF Figures 2.1, 2.2, 2.3, 2.5 and 2.6

**100 series, 41 568 points.** Every BSAF Phase II participant's predicted
unit 1 RPV pressure, drywell pressure (0–23 h and 23–500 h), in-vessel
hydrogen, water injection, fission-product release from fuel, and Cs and I
deposited on drywell structures and dissolved in the suppression-chamber
water, plus the **measured** RPV and drywell pressures plotted with them.
(54 series and 21 303 points from Figs 2.1, 2.2, 2.3b and 2.5; **46 series and
20 265 points added later on 2026-10-29** from Figs 2.3a and 2.6.)

These are the curves a unit 1 plant model can be verified against. Until now
the workspace only had bands read by eye off these figures
([`../nea7525-bsaf-fukushima-daiichi-bands.md`](../nea7525-bsaf-fukushima-daiichi-bands.md)
§1.4–§1.8). Several of those eye-read values were wrong, and have been
corrected there (see "Eye-read values that the digitisation corrected").

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). The code
> curves are code predictions of a real accident, digitised from summary
> figures. The measured points are operator and plant readings as BSAF plots
> them. Neither is a licensing input, and neither is authoritative for any
> facility.

---

## Provenance

| Field | Value |
|---|---|
| Source | OECD Nuclear Energy Agency (2021), *Benchmark Study of the Accident at the Fukushima Daiichi Nuclear Power Plant (BSAF Project), Phase II: Summary Report*, NEA No. 7525. Operating agent: Institute of Applied Energy (IAE), Japan |
| Figures | 2.1 A and B (RPV and drywell pressure, printed p.17), 2.2 (in-vessel H2, p.19), 2.3 A and B (drywell pressure 0–500 h, alternative water injection, p.20), 2.5 A and B (Xe and I released from fuel, p.21), 2.6 A–D (Cs and I deposited on D/W structures and dissolved in S/C water, p.22) |
| Copy held | the maintainer's private repository, `propreitrary-literature-theodore`, `reports/nea-7525-bsaf-fukushima-daiichi-summary-report.pdf` |
| Access tier | **Proprietary** (© OECD 2021). The PDF and the figure rasters stay private. Extracted coordinates are facts, published here with acknowledgement, as the OECD terms allow |
| Digitised | 2026-10-29, `kovan-cli digitise` (the workspace's own digitiser, built `--release`) with the pre- and post-processing in [`tools/`](tools), reusing the Figs 5.1–5.4 pipeline in [`../nea7525-bsaf-cs137-release-rates/`](../nea7525-bsaf-cs137-release-rates) |
| Operator | AI agent, automatic. **Not reviewed by a human.** |

**Acknowledgement.** Source and copyright owner: OECD Nuclear Energy Agency,
Benchmark Study of the Accident at the Fukushima Daiichi Nuclear Power Plant,
NEA No. 7525, © OECD 2021.

---

## Files

- **`all-series.csv`**: everything in long format,
  `figure, series, time_h, quantity, value, basis`. Start here.
- **`fig<N>-<series>.csv`**: one series each, `time_h,<quantity>`. Quantities
  and units: `rpv_pressure_mpa_abs` and `drywell_pressure_mpa_abs` (MPa
  absolute), `h2_generated_kg` (kg), `injection_kg_per_s` (kg/s),
  `xe_release_fraction` and `i_release_fraction` (fraction of the initial core
  inventory released from fuel), `cs_dw_deposited_fraction` and
  `i_dw_deposited_fraction` (Fig 2.6 A, B: fraction of the initial core
  inventory on drywell structures), `cs_sc_water_fraction` and
  `i_sc_water_fraction` (Fig 2.6 C, D: fraction dissolved in the S/C water).
  Fig 2.3a reuses `drywell_pressure_mpa_abs`.
- **`fig2.1a-measurement.csv`, `fig2.1b-measurement.csv`,
  `fig2.3a-measurement.csv`**: the measured data.
  They carry a third column, `basis`, saying how each marker centre was found
  (Method, step 9). **The measured series matter most**: they are the only
  rows here that can validate anything.
- **`fig<N>-digitised.png`**: the digitised data **replotted on its own**, not
  overlaid on the source figure, so it can be published while the copyrighted
  figure stays private. Put it beside the printed figure to check by eye.
- **`tools/`**: the whole pipeline, so this is reproducible rather than
  asserted. `curves-unit1.tsv` holds every figure's axis pixels, calibration
  and legend colours.

Time is written to 3 decimals. One pixel is 0.0125–0.020 h (0.4 h on Figs
2.3a, 2.3b, 2.6a and 2.6b; 0.029 h on 2.6c and 2.6d), so the third decimal is
already below a pixel. Values are written to the precision one pixel
supports: 0.0098 MPa per pixel on Fig 2.1a (3 decimals), 0.0012 MPa on 2.1b
and 2.3a (4), 1.42 kg on 2.2 (1), 0.023 kg/s on 2.3b (2), 0.0011 on 2.5 (4),
and 0.0006–0.0009 in mass fraction on 2.6a–d (4).

**Not written, and why:**
- **`fig2.3b-snl.csv`**: SNL is black and injects 0 kg/s, so it is drawn on
  the black x axis and cannot be separated from it.
- **`fig2.3b-kaeri.csv`**: no pixel of KAERI's cyan survives. It is at 0 kg/s
  under the curves drawn after it.
- ~~**Fig 2.3a** (PCV pressure over 0–500 h) and **Fig 2.6** (fission products
  deposited in the D/W and dissolved in the S/C): **not digitised**, for lack
  of time.~~ **CORRECTED 2026-10-29**: both have since been digitised (every
  code curve on each panel, and Fig 2.3a's measured data). Fig 2.3a's markers
  do overlap as Fig 2.1b's do, and are published as an envelope the same way.
  What is still missing from them is listed under Limitations 9–13.
- **Fig 2.4** (concrete erosion) was not in the brief.

An empty file would have looked like a failed extraction.

---

## Method

1. **Extract each figure at native resolution** with `pdfimages` (embedded
   JPEGs at 449 ppi, 1 366–2 383 px wide). Pages are not re-rendered, because
   re-rendering only upsamples.
2. **Find the axes and calibrate from the tick marks** (`tools/axes.py`).
   These MATLAB figures draw only a left and a bottom axis ("box off"), with
   short inward ticks and no gridlines, except Fig 2.3b. The tick finder
   cannot tell a tick from a curve running perpendicular to the axis. Marks
   like that are assigned no label (printed as such, so the assignment can be
   audited), and ticks hidden under curves are simply absent. The axis-line
   centre is used as the "0" tick where the label sits on the axis.
3. **Locate each legend box** (`tools/legendbox.py`). `calib.legend_box`
   from the Cs-137 pipeline assumes a full frame, and here it picked up flat
   curves instead.
4. **Take each series' colour from its own legend swatch** (`tools/swatch.py`,
   changed: light curves now count as ink, and the swatch core is the row
   farthest from paper white. The Cs-137 version dropped yellow and light
   green, and a max-saturation rule picked dark green's paler edge).
5. **Mask the legend box.** On all five panels (Figs 2.1, 2.2, 2.3b and 2.5; not Fig 2.6c or 2.6d, see Limitations 9) the legend sits in a region no
   curve enters (checked on each figure), so no data is lost under it, unlike
   Figs 5.1–5.4.
6. **Isolate each curve by nearest-colour classification** with a **30-unit
   cap** (`tools/isolate_all.py`, changed so that the non-curve references
   can be set per figure). Black is a *curve* here (SNL), so it is not a
   background reference. The references are paper, gridline greys, an
   anti-aliased-black grey, and the measurement-marker fill and rim colours,
   so marker pixels are not absorbed by the nearest curve. The reason for
   classification rather than per-colour thresholds is in the Cs-137 README
   ("Why nearest-colour classification"). It matters even more here: nine
   curves include IAE blue (0,0,215) and NRA purple (107,0,216), and dark
   green (0,58,0) and black.
7. **Prepare each isolated image for the tracer** (`tools/run_unit1.py`):
   white out the inward tick marks at the calibrated tick positions (they are
   black, so they classify as SNL), and draw a black frame over the axis
   lines, because `kovan-cli digitise` needs a frame to detect. The top and
   right edges go 9 px outside the axes, because several curves run exactly
   along release fraction 1 or to the axis end. On Fig 2.3b the bottom edge
   also goes 9 px below the axis, so that 0 kg/s stays traceable.
8. **Trace** with `kovan-cli digitise`: linear axes, explicit `--x-ref` /
   `--y-ref` from the step-2 fit, `--curve-tolerance 30`, `--step 2`,
   `--inset 5`, continuity strategy. Then **drop isolated points**
   (`tools/despeckle.py`, window ±0.26 h, or ±4 h on Fig 2.3b): 142 of 20 190
   points on Figs 2.1/2.2/2.5, and 8 of 1 699 on 2.3b.
9. **Measured markers are not curves**, and a column tracer is the wrong
   instrument for them: it returns one value per column across a 27–44 px
   marker. `tools/markers.py` classifies the marker fill (and rim) against all
   the curve colours, and places marker centres at the **bounding-box centre**
   (MATLAB centres markers on their bounding box):
   - **Fig 2.1a**: two orange-red squares, 27 × 27 px. The bounding box is
     used where its extent matches the marker size. Where a curve crosses one
     edge, the centre is anchored to the edge that borders paper
     (`basis = square-x:full-y:anchored-low`).
   - **Fig 2.1b**: teal triangles with a light-green rim, 44 × 34 px
     including the rim (measured on the one isolated marker, at 10.3 h). From
     ~13 h they overlap every ~7 px, and every code curve is drawn on top of
     them, so most single markers cannot be separated. **What is published is
     the envelope of the marker cloud** (`envelope` mode). In each 22 px
     (0.41 h) bin, `upper` is the highest marker top that borders paper, plus
     H/2, and `lower` is the lowest marker base that borders paper, minus H/2.
     An `upper` point lower than a `lower` point in the same bin is impossible
     for a marker cloud. It means a crossing curve cut the marker and the cut
     was taken for a tip. Such points are kept but labelled `upper-suspect`
     (4 of 37), and should not be used.
10. **Round and assemble** (`tools/finalise_unit1.py`).

### Method, Figs 2.3a and 2.6 (added 2026-10-29)

The same pipeline, extended rather than copied: every change is opt-in per
figure, so re-running it reproduces the earlier 54 CSVs and the earlier
21 303 rows of `all-series.csv` **byte for byte** (checked by `cmp` on
2026-10-29). The differences:

- **Images.** Fig 2.3a is `u1-022-003.png` (1 447 × 1 093 px) and Fig 2.6 A–D
  are `u1-024-002.png` to `u1-024-005.png` (1 423–1 460 px wide), from the
  same `pdfimages` run. Fig 2.6 B is visibly softer than the other three
  panels (a resampled raster), which matters below.
- **Calibration, Fig 2.3a: ticks.** Its x axis is drawn mid-grey (≈163 after
  JPEG), above `axes.py`'s darkness threshold, which then found the legend
  border instead. `axes.py` now takes `DARK` from the environment (default
  150, unchanged); Fig 2.3a uses `DARK=180`. The 0.6 MPa tick is hidden by
  curves.
- **Calibration, Fig 2.6: gridlines** (`tools/grid_lin.py`, new). These
  panels draw very light gridlines (≈239) at every label, while curves at zero
  lie on the x axis and break `axes.py`'s axis and tick finder. Gridline
  candidates that are legend-box edges are printed and assigned no label.
  The extrapolated zero lines are then compared with the drawn axis lines as a
  separate check.
- **Colours.** From each panel's own legend swatches (`swatch.py`), nearest-
  colour classification, 30-unit cap, **except Fig 2.6b**: its legend is
  blurred paler than its curves (KAERI swatch 99,239,240 against a plot core of
  0,226,225; IBRAE 33,113,32 against 1,76,0), so no swatch colour there is a
  usable reference. Its rows carry **Fig 2.6a's swatch colours** (same nine
  codes in the same order, sharp legend) and a **90-unit cap**, because the
  soft plot cores still sit up to 86 units from those (JAEA core 62,251,60
  against 1,240,0). Both numbers were measured on the raster before tracing.
  The first trace with that cap then took the black axis's anti-aliased grey
  (≈58,58,58, 80 units from VTT brown) as VTT, from 120 to 175 h, and the
  grey edge of SNL's black line as VTT near 30 h (a false 0.26 peak). The
  axis-edge grey is now a non-curve reference on Fig 2.6b (as it already is in
  `isolate_all.py`'s default list), and VTT ends at 119.5 h with a 0.108
  peak, as drawn. Fig 2.3a's marker rim is lighter and yellower than Fig
  2.1b's; its measured rim colours (≈120,235,70 and ≈197,251,167) are added
  to the non-curve references there.
- **Zero-level curves (Fig 2.6).** Every panel has curves at exactly 0, on the
  x axis, so the frame bottom goes 9 px below the axis, as on Fig 2.3b, and
  the x ticks are left in place. **Except for SNL (black)**: there the axis
  line itself would be traced as SNL, so SNL keeps the frame on the axis and
  its zero segments are lost (Limitations 10).
- **Despeckle windows** of about 10–13 px, as before: 4.0 h on Figs 2.3a,
  2.6a and 2.6b (110 of 11 559 points dropped on the first run), 0.38 h on
  2.6c and 2.6d (6 of 8 792).
- **Measured markers, Fig 2.3a.** The same teal triangles as Fig 2.1b, smaller
  on this raster: **37 × 35 px** including the rim, measured on the legend
  marker (rim apex row 74, rim base row 108, rim columns 1 060–1 096; fill
  32 × 32). From 165 h they overlap every 2–3 px and every curve crosses
  them, so `envelope` mode is used again, with bins of W/2 = 18 px = **7.1 h**
  and the same `upper-suspect` self-check (1 point). Every envelope point was
  then inspected on the private overlay, and one more was labelled by the
  operator: a `lower` point at 358.7 h that sits on the crossing of the IAE
  (blue) and JAEA (green) curves, whose blend mimics teal fill over a
  pale-green rim (checked on a 5× zoom). It is kept, as `lower-suspect`. An
  automatic test for this (fill within 5 rows above a base) was tried first
  and **rejected, not tuned**: it also flagged 13 bases that are genuine.
  Note the **time** of an envelope point: an `upper` point is at the apex
  column, so its time is the marker's; a `lower` point is at the column where
  the lowest base edge was found, anywhere along a base W px wide, so its time
  is uncertain by up to **±W/2: ±7.3 h on Fig 2.3a, ±0.41 h on Fig 2.1b**
  (this applies to the Fig 2.1b envelope too, and was not stated before).
  It is why two `lower` points sit at 501.2 and 507.9 h, past the 500 h axis
  end. One `upper` point is also past it, at 501.6 h (4 px beyond the
  axis-end tick); whether a marker is centred just beyond the axis limit or
  the apex is offset by a pixel or two cannot be told from the raster.

---

## Verification

Every number below was produced by running the tools on 2026-10-29.

### 1. Calibration against the drawn ticks (`tools/axes.py`) and gridlines (`tools/grid_lin.py`)

The tick labels are evenly spaced, so the detected ticks must be evenly
spaced pixels. A least-squares line through each axis's ticks:

| Panel | Axis | Ticks used | Measured spacing (px) | Fitted px/label step | Worst residual |
|---|---|---|---|---|---|
| 2.1a | x (0–20 h) | 5 | 257.0, 255.5, 255.0, 256.0 | 255.75 | 0.65 px |
| 2.1a | y (0–8 MPa) | 6 (5, 6, 7 hidden by curves) | 103.0, 102.0, 102.5, 103.5, then 409.5 over 4 steps | 102.56 | 0.60 px |
| 2.1b | x | 5 | 265.5, 266.5, 266.5, 265.0 | 266.00 | 0.60 px |
| 2.1b | y (0–1 MPa) | 6 | 165.5, 164.5, 164.5, 165.0, 164.5 | 164.76 | 0.39 px |
| 2.2 | x (0–25 h) | 6 | 400.0, 398.5, 398.5, 399.5, 399.0 | 399.01 | 0.60 px |
| 2.2 | y (0–1400 kg) | 8 | 140.5, 140.5, 141.0, 141.0, 140.5, 141.0, 140.0 | 140.72 | 0.38 px |
| 2.5a | x (0–15 h) | 3 (5 h hidden; one curve mark at 447 px rejected) | 780.5 over 2 steps, 392.0 | 390.75 | 0.75 px |
| 2.5a / 2.5b | y (0–1) | 6 | 185.5, 183.0, 186.0, 185.0, 184.5 | 184.80 | **1.10 px** |
| 2.5b | x | 4 | 390.5, 390.0, 392.0 | 390.75 | 0.75 px |
| 2.3b | x (50–500 h) | 6 (four hidden; three curve marks rejected) | 249.5, 250.5, 124.5, 250.5, 249.5 | 124.98 per 50 h | 0.31 px |
| 2.3b | y (0–20 kg/s) | 5 | 219.5, 220.0, 220.0, 221.0 | 220.10 | 0.50 px |
| 2.3a | x (50–500 h), ticks | 10 | 127.0, 127.5, 127.5, 127.0, 127.5, 127.5, 127.0, 127.5, 127.5 | 127.34 per 50 h | 0.19 px |
| 2.3a | y (0–1 MPa), ticks | 5 (0.6 hidden by curves) | 163.5, 163.5, then 328.0 over 2 steps, 164.0 | 163.82 per 0.2 MPa | 0.37 px |
| 2.6a | x (100–400 h), gridlines | 4 (two legend edges rejected) | 239.0, 238.0, 239.0 | 238.60 per 100 h | 0.30 px |
| 2.6a | y (0.2–0.6), gridlines | 5 (0.1 under the IAE curve; two legend edges rejected) | 144.5, 144.0, 144.5, 144.0 | 144.25 per 0.1 | 0.15 px |
| 2.6b | x (100–500 h), gridlines | 5 (two legend edges rejected) | 235.5, 237.5, 236.7, 235.3 | 236.42 per 100 h | 0.72 px |
| 2.6b | y (0.1–0.5), gridlines | 5 (two legend edges rejected) | 183.0, 181.0, 181.5, 181.0 | 181.55 per 0.1 | 0.75 px |
| 2.6c | x (5–30 h), gridlines | 6 (two legend edges rejected) | 171.0, 170.0, 172.0, 171.0, 171.0 | 171.03 per 5 h | 0.82 px |
| 2.6c | y (0.1–0.8), gridlines | 8 (three legend edges rejected) | 112.0, then 110.0 × 6 | 110.17 per 0.1 | **1.17 px** |
| 2.6d | x (5–30 h), gridlines | 6 (one legend edge rejected) | 174.0, 173.0, 174.0, 174.0, 174.0 | 173.77 per 5 h | 0.45 px |
| 2.6d | y (0.1–0.6), gridlines | 6 (a partial line at 935 px and a legend edge rejected) | 134.0, 133.5, 134.5, 133.5, 133.5 | 133.83 per 0.1 | 0.42 px |

Fig 2.3b also has vertical gridlines, found independently at 125.0, 124.5,
250.0, 125.0 and 126.0 px spacing: they agree with the ticks. On Fig 2.6 the
gridline fits were checked against the drawn axis lines, which were not used
in them: the extrapolated zero falls within **0.1–1.1 px** of the axis line on
every panel (x: 166.0 vs 165.5, 164.4 vs ≈163.5, 165.7 vs 165.5, 170.1 vs
170; y: 1 046.35 vs rows 1 046–1 049 on 2.6b, 941.0 vs 937–944 on 2.6c, 939.7
vs 938–942 on 2.6d; on 2.6a the axis row is covered by zero-level curves).
Fig 2.3a's axis starts at **22.97 h**, by extrapolating its ticks to the y
axis. **Every residual is at most 1.17 px** (1.1 px before Figs 2.3a and 2.6
were added). The worst per panel converts to at most 0.0064 MPa (2.1a),
0.0005 MPa (2.1b), 0.54 kg (2.2), 0.0012 in release fraction (2.5), 0.011
kg/s (2.3b), and at most 0.013 h on the 0–25 h time axes; and 0.0005 MPa and
0.075 h (2.3a), 0.0001 / 0.13 h (2.6a), 0.0004 / 0.30 h (2.6b), 0.0011 /
0.024 h (2.6c), 0.0003 / 0.013 h (2.6d). The worst, on Fig 2.5's y axis, is reported rather
than hidden.

### 2. Independent checks against the report's text and tables

The text and tables were read from different pages and are not inputs to the
trace, so they can disagree with it. `tools/checks_unit1.py` prints them all.

| Statement in the report | Digitised | Verdict |
|---|---|---|
| p.17: RCS pressure "only manually measured twice by operators between 5 and 12 hours", showing depressurisation in that window | Two markers: **7.004 MPa at 5.345 h** and **0.900 MPa at 11.992 h**. The second marker's bottom edge is crossed by curves, so its centre is anchored to the top edge; the plain bounding-box centre would give 0.920 MPa | **Agrees**: both readings are inside 5–12 h, falling from ~7 to <1 MPa |
| Table 2.1: KAERI main-steam-line failure at **5.0 h**, giving immediate depressurisation | KAERI's RPV trace is last above 7 MPa at **5.23 h** and first below 3 MPa at **5.35 h** | **Disagrees by 0.2–0.35 h** (12–18 px, well beyond calibration error). Either the plotted curve or the table entry is offset. Not resolved here |
| Table 2.1: SNL MSL failure at **6.1 h** | last >7 MPa at **6.09 h**, first <3 MPa at **6.17 h** | **Agrees** |
| Table 2.2 and p.18: CIEMAT (**11.58 h**) and VTT (**11.42 h**) lower-head failure "coincident with ... the largest measured PCV pressure at around 11.5 hours" | CIEMAT RPV first <3 MPa after 10 h at **11.60 h**; VTT at **11.52 h** (its fall is a ramp from 10.8 h, so "first <3 MPa" lags the failure). Largest measured D/W marker centre: **0.851 MPa at 11.72 h** | **Agrees** within 0.1–0.2 h |
| p.19: "the data predicted by most participants between 15 hours and the PCV venting time (around 24 hours) is consistent with the measurements" | Measured marker centres 16–23 h: **0.738–0.775 MPa**, mean 0.757 (n = 16). Code means over their visible 16–23 h trace: IAE 0.764, IBRAE 0.755, IRSN 0.736, JAEA 0.756 (4 points only), NRA 0.749, SNL 0.747, VTT 0.768, **KAERI 0.829**. CIEMAT is not visible after 14.2 h | **Agrees**: 7 of 8 are within the measured range ±0.02 MPa. KAERI's visible trace ends at 19.1 h while it is still decaying from its 1.02 MPa peak |
| p.21: release from fuel reaches an asymptote "which in most cases exceeded 80%" | Last traced value above 0.80 for **9 of 9 (Xe)** and **8 of 9 (I)**. The exception is SNL iodine, **0.775 at 15 h** | **Agrees** |
| p.19 (Fig 2.3b): calculations capturing the ~270 h trend "agree on an injection rate lower than 4.5 kg/s" | Maxima from 275 h: IAE 4.15, JAEA 3.70, IBRAE 2.89, CIEMAT 1.92, IRSN 0.95, **NRA 9.24 kg/s** (270–294 h) | **Agrees for 5 of 6**. NRA exceeds it |

The caller's brief also expected "drywell measured ~0.6–0.75 MPa near
12–20 h". **The text states no drywell numbers.** The markers themselves show
0.60 MPa at 10.3 h, **0.85 MPa at 11.7–12.3 h**, 0.77–0.79 MPa at 13.5–14 h,
and 0.74–0.80 MPa from 15 h to 23 h. So the 0.6–0.75 range is not what the
figure shows near 12 h.

#### Figs 2.3a and 2.6 (added 2026-10-29, `tools/checks_unit1_late.py`)

Besides the text (pp.19–22), these figures can be checked against **other
figures of the same report**, traced from different rasters: Fig 2.1b plots
the same measured drywell series up to 23 h, Fig 2.3b the injections that are
said to cause the 270 h pressure rise, and Fig 2.5b the iodine released from
fuel, which bounds what can be deposited or dissolved. The measured values
below are envelope **centres**, excluding the 2 suspect points.

| Statement or relation | Digitised | Verdict |
|---|---|---|
| p.19: MCCI gas "drove a pressure rise in the PCV until 50 hours, when the trend reversed and decreased for more than 200 hours until it sharply started increasing again at 270 hours" | Measured: after the 24 h vent, a rise to **0.596 MPa at 54.0 h**; then decrease to a minimum of **0.153 MPa at 257.0 h** (upper envelope at most 0.183 MPa over 240–270 h); the first upper point clearly above that (> 0.213 MPa) is at **275.1 h**; peak **0.411 MPa at 302.5 h** | **Agrees.** The peak is at 54 h rather than 50 h, and the rise is resolved at 275 h rather than 270 h, both inside one 7.1 h envelope bin. There are **no markers at all between 76.4 and 168.3 h**, so "decreased for more than 200 hours" is only partly measured |
| Cross-figure, same measured series: Fig 2.1b (ends 23.3 h) and Fig 2.3a (starts 23.0 h) | Fig 2.1b upper centres at 21.5–23.3 h: **0.765, 0.766 MPa**. Fig 2.3a upper centres before the vent: **0.752 at 20.2 h, 0.763 at 21.8 h** | **Agrees within 0.003–0.014 MPa**, from two rasters calibrated independently (one Fig 2.1b pixel is 0.0012 MPa, one Fig 2.3a pixel 0.0012 MPa) |
| p.19: "a few calculations captured the changing trend [at ~270 h] by imposing an effective water injection" (Fig 2.3b) | Rise in drywell pressure from 265 h to the 272–330 h maximum: CIEMAT +0.249, IBRAE +0.225, IRSN +0.200, IAE +0.138, JAEA +0.112, NRA +0.074, **SNL +0.070**, KAERI +0.007 MPa. Fig 2.3b injections over 250–330 h: IAE 4.15, JAEA 3.72, IBRAE 3.64, IRSN 0.95, CIEMAT 0.75 kg/s (CIEMAT's trace starts only at 289 h), NRA 18.4 kg/s (at 251 h, before the window); KAERI and SNL not traceable on Fig 2.3b | **Mostly agrees.** KAERI neither injects visibly nor rises. **SNL rises by 0.07 MPa at ≈272 h** although no SNL injection could be separated on Fig 2.3b (SNL is black and on the axis there), so SNL may inject at ≈272 h at a rate the figure hides. CIEMAT rises most while its visible injection is smallest; its injection before 289 h is hidden on Fig 2.3b. Not resolved |
| p.21: "At 35 hours, 10–80%, roughly, of Cs and I was estimated to have been scrubbed into the S/C water" (Fig 2.6 C, D) | Cs at 34.55–35 h: JAEA 0.787, NRA 0.439, CIEMAT 0.364, SNL 0.328, VTT 0.257, KAERI 0.225, IRSN 0.166, **IAE 0.079, IBRAE 0.000**. I: CIEMAT 0.620, JAEA 0.582, VTT 0.561, NRA 0.544, SNL 0.410, IRSN 0.219, IAE 0.155, KAERI 0.138, **IBRAE 0.001** | **Agrees roughly**, as worded: 7 of 9 (Cs) and 8 of 9 (I) lie in 0.10–0.80. **IBRAE scrubs nothing** (its curve lies on the axis throughout), and IAE's Cs is 7.9 % |
| p.21: "significant deposition was predicted on the D/W structures and some remobilisation was noted in some calculations" (Fig 2.6 A, B) | Deposit falls below half of its maximum: Cs for IAE (by 23 h), NRA (27 h), VTT (51 h), SNL (88 h), KAERI (148 h); I for NRA (28 h), VTT (62 h), SNL (96 h), KAERI (151 h), IAE (437 h). Largest deposits: IBRAE **0.700** Cs and **0.552** I, both along the top edge of the axis | **Agrees**: five codes of nine show remobilisation of each element |
| Cross-figure mass balance, iodine at 13–16 h: D/W deposit (2.6b) + S/C water (2.6d) ≤ released from fuel (2.5b) | Sums 0.255 (IAE) to 0.820 (JAEA); the largest traced 2.5b value up to 15 h (a lower bound on the release, which cannot fall) is 0.775 (SNL) to 1.00. **All nine sums are below their release bound** | **Consistent, but weak**: the margin is 0.20–0.75 for eight codes and only 0.14 for JAEA, and the bound would catch only a gross error, such as a series assigned to the wrong panel |
| D/W deposit + S/C water ≤ 1 at 35 h | Cs 0.188 (IAE) to 0.853 (JAEA); I 0.268 (IAE) to 0.828 (JAEA) | **Consistent** (weak, as above) |
| Text, Fig 2.6 (p.21): "most analyses predicted less than 10% airborne" | Not plotted | **Not checkable** here |

### 3. Comparison with the eye-read bands (`../nea7525-bsaf-fukushima-daiichi-bands.md`)

The values that the digitisation corrected are now struck through there, with
**CORRECTED 2026-10-29 (digitised)**:

| Bands file | Eye-read | Digitised | |
|---|---|---|---|
| §1.4 H2 lowest (JAEA) | ≈400 kg | 398.1 kg | right |
| §1.4 H2 highest (IBRAE) | ≈1 310 kg | 1 315.0 kg | right |
| §1.4 H2 "bulk" | 600–1 000 kg | six codes 604–943 kg; **IAE 1 121.6 kg** | **wrong**: IAE is outside it |
| §1.4 H2 complete / second step | complete by 10–15 h; small second step near 15 h "in several" | 95 % of plateau at 7.1–15.0 h; late steps CIEMAT and VTT ≈12.6 h, **NRA alone** at 14.5–15.3 h | **wrong** in detail |
| §1.5 D/W measurements | "two points plotted" | **dozens of markers** from 10.3 h on, dense from 15 h | **wrong** |
| §1.5 D/W calculated peaks | 0.8–1.0 MPa, 10–16 h | 0.747 (SNL, no peak) to **1.024 MPa (KAERI, 15.0 h)**, at 11.5–15.7 h | **wrong** at both ends |
| §1.5 D/W plateau by 20 h | 0.75–0.8 MPa | visible traces at 20 h: 0.735–0.769 MPa | **slightly wrong** (low end) |
| §1.6 injection before 270 h | "taken to be 0" | NRA ~1.7 kg/s at 77–102 h, ~0.1 kg/s at 175–197 h and **11.8, 15.3 then 18.4 kg/s from 198 to 272 h**; JAEA ~0.1 kg/s at 77–100 h; IBRAE from ~252 h | **wrong** |
| §1.8 release onset | 3–6 h | first >0.05 at 2.89 h (JAEA) to 5.24 h (KAERI); **IBRAE Xe 6.96 h** (one step) | **wrong** at both ends |
| §1.8 asymptote | > 80 %, most 0.9–1.0 | as in the check above | right |
| §1.5 0–500 h measured, density | "dense" | **no markers between 76 and 168 h** | **wrong** (added 2026-10-29) |
| §1.5 0–500 h measured, values | fall from ≈0.8 to ≈0.15–0.2 MPa by 250 h, rise to ≈0.4 near 300 h, slow decline | centres 0.763 MPa at 21.8 h (tips 0.78); ≈0.52 by 25–29 h; **back up to 0.596 at 54 h**; 0.153–0.183 at 230–270 h; 0.411 at 302.5 h; 0.17–0.21 at 480–500 h | **right in outline**, but omits the 50 h rise the text states, and "≈0.8" is the marker tips (added 2026-10-29) |
| §1.5 0–500 h, "well reproduced" | all codes, implied | code means within the measured range ±0.02 MPa: 4 of 8 at 170–260 h, 3 of 7 at 330–420 h | **wrong** for about half the codes (added 2026-10-29) |
| §1.8 Cs on D/W structures | ≈0.02–0.7, **JAEA highest** | ≈0 (KAERI, VTT, NRA 0.003) to **0.70, IBRAE**; JAEA's maximum is 0.073 | **wrong** (added 2026-10-29) |
| §1.8 Cs and I in S/C water at 35 h | ≈10–80 % (the report's own words) | Cs 0.079–0.787, I 0.138–0.620, **IBRAE 0** for both | right, roughly; digitised values added beside it, not struck (added 2026-10-29) |

### 4. Round-trip check, and why it is weak

`tools/quality_unit1.py` reprojects every point to a pixel and tests whether
the **original** figure is within 30 RGB units of that series' legend colour
in the 3 × 3 neighbourhood. **52 of 52 code series pass at 100 %**, and so do
the **45 of 45** added from Figs 2.3a and 2.6 (Fig 2.6b at its 90-unit
tolerance, so weaker still). This is
**partly circular**: the tracer only accepted pixels within the same
tolerance. It shows that the calibration round-trips and that nothing landed
on axis or blank paper. It **cannot** show that the right curve was followed,
and it passed the blend artefacts listed under Limitations (including Fig
2.6b NRA's IAE segment). It is reported
because it was run, not because it is strong. The checks in section 2 are the
evidence.

The overlays (digitised points drawn on the copyrighted figures) are in the
private repository, `derived-verification/nea7525-bsaf-unit1-digitisation/`.

### Per-series quality

`N` is the point count, `SPAN` the traced time range (h), and `COV` the
coverage: the share of pixel columns from the first traced point **to the end
of the axis** (every code curve in Figs 2.1, 2.2 and 2.5 is drawn to the axis
end) that carry a traced point. On Fig 2.3b, where curves visibly stop early,
coverage runs to the last traced point, and so it does for VTT on Figs 2.3a,
2.6a and 2.6b, whose curve visibly ends at ≈120 h (set before grading; every
other curve on Figs 2.3a and 2.6 is drawn to the axis end). Low coverage means the curve is
hidden under later-drawn curves. No tracer can recover that from the raster:
**use only the time ranges actually present**. `SPK` counts out-and-back
excursions of 1–3 points larger than 8 % of full scale (suspected crossing
artefacts, not removed). `RT` is the round-trip share (%).

Grades: **A** COV ≥ 0.85 and SPK ≤ 2 · **B** COV ≥ 0.60 and SPK ≤ 6 · **C**
otherwise (use the visible segments only). There is no integral check here as
there was for Cs-137, so coverage stands in for it. The first run measured
coverage to the *last traced point*. That scored IAE on Fig 2.5a, whose whole
tail at 1.0 is hidden, as 1.00, so the stricter definition replaced it before
the grades were recorded.

| Fig | Series | N | SPAN (h) | COV | SPK | RT | Grade |
|---|---|---:|---|---:|---:|---:|---|
| 2.1a | CIEMAT | 112 | 6.32–13.44 | 0.30 | 0 | 100.0 | **C** |
| 2.1a | IAE | 201 | 0.50–15.94 | 0.37 | 1 | 100.0 | **C** |
| 2.1a | IBRAE | 248 | 1.98–13.91 | 0.49 | 1 | 100.0 | **C** |
| 2.1a | IRSN | 463 | 2.65–22.98 | 0.92 | 0 | 100.0 | **A** |
| 2.1a | JAEA | 227 | 0.07–13.95 | 0.42 | 0 | 100.0 | **C** |
| 2.1a | KAERI | 347 | 0.77–18.05 | 0.64 | 0 | 100.0 | **B** |
| 2.1a | NRA | 192 | 0.07–16.72 | 0.36 | 1 | 100.0 | **C** |
| 2.1a | SNL | 425 | 0.07–22.98 | 0.76 | 0 | 100.0 | **B** |
| 2.1a | VTT | 492 | 0.81–22.98 | 0.94 | 0 | 100.0 | **A** |
| 2.1a | measurement | 2 | 5.35, 11.99 | — | — | — | **A** (two isolated markers; one centre anchored) |
| 2.1b | CIEMAT | 183 | 4.58–14.17 | 0.40 | 0 | 100.0 | **C** |
| 2.1b | IAE | 395 | 2.96–22.17 | 0.78 | 0 | 100.0 | **B** |
| 2.1b | IBRAE | 390 | 2.85–22.96 | 0.82 | 1 | 100.0 | **B** |
| 2.1b | IRSN | 377 | 3.64–22.77 | 0.80 | 0 | 100.0 | **B** |
| 2.1b | JAEA | 319 | 1.91–16.12 | 0.59 | 0 | 100.0 | **C** |
| 2.1b | KAERI | 392 | 4.09–19.09 | 0.79 | 0 | 100.0 | **B** |
| 2.1b | NRA | 447 | 0.14–22.96 | 0.78 | 1 | 100.0 | **B** |
| 2.1b | SNL | 589 | 0.07–22.96 | 0.98 | 0 | 100.0 | **A** |
| 2.1b | VTT | 473 | 0.93–22.85 | 0.89 | 0 | 100.0 | **A** |
| 2.1b | measurement | 37 | 10.12–23.28 | — | — | — | **B** (envelope, not single markers; 4 `upper-suspect`) |
| 2.2 | CIEMAT | 654 | 4.00–24.98 | 0.80 | 0 | 100.0 | **B** |
| 2.2 | IAE | 789 | 4.53–24.98 | 0.97 | 0 | 100.0 | **A** |
| 2.2 | IBRAE | 836 | 3.52–24.98 | 0.98 | 0 | 100.0 | **A** |
| 2.2 | IRSN | 861 | 3.25–24.98 | 1.00 | 0 | 100.0 | **A** |
| 2.2 | JAEA | 876 | 2.57–24.98 | 0.99 | 0 | 100.0 | **A** |
| 2.2 | KAERI | 798 | 4.42–24.98 | 0.98 | 0 | 100.0 | **A** |
| 2.2 | NRA | 843 | 3.67–24.98 | 1.00 | 0 | 100.0 | **A** |
| 2.2 | SNL | 849 | 3.55–24.98 | 1.00 | 0 | 100.0 | **A** |
| 2.2 | VTT | 833 | 3.45–24.98 | 0.98 | 0 | 100.0 | **A** |
| 2.3b | CIEMAT | 228 | 289.30–498.94 | 0.87 | 0 | 100.0 | **A** |
| 2.3b | IAE | 278 | 274.90–498.94 | 0.99 | 0 | 100.0 | **A** |
| 2.3b | IBRAE | 200 | 254.90–478.14 | 0.76 | 0 | 100.0 | **B** |
| 2.3b | IRSN | 37 | 275.70–307.71 | 0.99 | 0 | 100.0 | **A** |
| 2.3b | JAEA | 75 | 78.86–310.11 | 0.27 | 0 | 100.0 | **C** |
| 2.3b | NRA | 289 | 77.26–379.72 | 0.77 | 0 | 100.0 | **B** |
| 2.3b | VTT | 109 | 33.25–119.67 | 1.00 | 0 | 100.0 | **A** |
| 2.5a | CIEMAT | 293 | 3.84–13.82 | 0.71 | 0 | 100.0 | **B** |
| 2.5a | IAE | 89 | 4.78–7.03 | 0.22 | 0 | 100.0 | **C** |
| 2.5a | IBRAE | 160 | 6.42–10.49 | 0.48 | 0 | 100.0 | **C** |
| 2.5a | IRSN | 426 | 2.17–14.97 | 0.88 | 0 | 100.0 | **A** |
| 2.5a | JAEA | 444 | 2.76–14.97 | 0.95 | 0 | 100.0 | **A** |
| 2.5a | KAERI | 364 | 4.40–14.10 | 0.90 | 0 | 100.0 | **A** |
| 2.5a | NRA | 401 | 3.53–14.25 | 0.91 | 0 | 100.0 | **A** |
| 2.5a | SNL | 371 | 3.45–13.00 | 0.83 | 0 | 100.0 | **B** |
| 2.5a | VTT | 401 | 3.32–13.97 | 0.90 | 0 | 100.0 | **A** |
| 2.5b | CIEMAT | 397 | 3.86–14.97 | 0.94 | 0 | 100.0 | **A** |
| 2.5b | IAE | 233 | 4.78–10.72 | 0.58 | 0 | 100.0 | **C** |
| 2.5b | IBRAE | 415 | 3.86–14.81 | 0.96 | 0 | 100.0 | **A** |
| 2.5b | IRSN | 450 | 2.30–14.97 | 0.93 | 0 | 100.0 | **A** |
| 2.5b | JAEA | 321 | 2.76–12.74 | 0.69 | 0 | 100.0 | **B** |
| 2.5b | KAERI | 358 | 4.42–13.89 | 0.88 | 0 | 100.0 | **A** |
| 2.5b | NRA | 429 | 3.55–14.97 | 0.99 | 0 | 100.0 | **A** |
| 2.5b | SNL | 441 | 3.45–14.97 | 0.99 | 0 | 100.0 | **A** |
| 2.5b | VTT | 444 | 3.32–14.97 | 0.99 | 0 | 100.0 | **A** |
| 2.3a | CIEMAT | 243 | 83.44–499.66 | 0.50 | 0 | 100.0 | **C** |
| 2.3a | IAE | 565 | 29.25–499.66 | 0.96 | 0 | 100.0 | **A** |
| 2.3a | IBRAE | 432 | 30.82–498.88 | 0.77 | 0 | 100.0 | **B** |
| 2.3a | IRSN | 522 | 24.54–499.66 | 0.93 | 0 | 100.0 | **A** |
| 2.3a | JAEA | 557 | 26.90–499.66 | 0.94 | 0 | 100.0 | **A** |
| 2.3a | KAERI | 575 | 30.04–499.66 | 0.98 | 0 | 100.0 | **A** |
| 2.3a | NRA | 422 | 24.54–375.58 | 0.72 | 0 | 100.0 | **B** |
| 2.3a | SNL | 581 | 25.33–499.66 | 0.96 | 0 | 100.0 | **A** |
| 2.3a | VTT | 64 | 51.24–119.56 | 0.89 | 0 | 100.0 | **A** (coverage to its last point: the curve ends at ≈120 h) |
| 2.3a | measurement | 99 | 20.22–507.91 | — | — | — | **B** (envelope, not single markers; 1 `upper-suspect`, 1 `lower-suspect`; no markers 76–168 h) |
| 2.6a | CIEMAT | 170 | 9.64–468.15 | 0.33 | 0 | 100.0 | **C** |
| 2.6a | IAE | 578 | 8.80–499.16 | 0.99 | 0 | 100.0 | **A** |
| 2.6a | IBRAE | 589 | 6.29–499.16 | 1.00 | 0 | 100.0 | **A** |
| 2.6a | IRSN | 568 | 7.96–499.16 | 0.98 | 0 | 100.0 | **A** |
| 2.6a | JAEA | 251 | 14.67–236.80 | 0.44 | 0 | 100.0 | **C** |
| 2.6a | KAERI | 217 | 21.37–211.65 | 0.39 | 0 | 100.0 | **C** |
| 2.6a | NRA | 584 | 4.61–499.16 | 0.99 | 0 | 100.0 | **A** |
| 2.6a | SNL | 589 | 6.29–499.16 | 1.00 | 0 | 100.0 | **A** |
| 2.6a | VTT | 131 | 9.64–119.45 | 1.00 | 0 | 100.0 | **A** (coverage to its last point: the curve ends at ≈120 h) |
| 2.6b | CIEMAT | 545 | 9.57–499.38 | 0.96 | 0 | 100.0 | **A** |
| 2.6b | IAE | 553 | 11.26–498.53 | 0.97 | 0 | 100.0 | **A** |
| 2.6b | IBRAE | 582 | 7.87–499.38 | 1.00 | 0 | 100.0 | **A** |
| 2.6b | IRSN | 559 | 9.57–499.38 | 0.98 | 1 | 100.0 | **A** |
| 2.6b | JAEA | 483 | 6.18–498.53 | 0.84 | 0 | 100.0 | **B** |
| 2.6b | KAERI | 195 | 14.64–178.76 | 0.34 | 0 | 100.0 | **C** |
| 2.6b | NRA | 575 | 4.49–499.38 | 0.99 | 0 | 100.0 | **A** (automatic; a 6-point segment at 246–251 h is IAE, see Limitations 12) |
| 2.6b | SNL | 117 | 6.18–111.93 | 0.22 | 0 | 100.0 | **C** |
| 2.6b | VTT | 133 | 6.18–119.54 | 1.00 | 0 | 100.0 | **A** (coverage to its last point: the curve ends at ≈120 h) |
| 2.6c | CIEMAT | 517 | 4.48–34.94 | 0.99 | 0 | 100.0 | **A** |
| 2.6c | IAE | 506 | 5.24–34.94 | 1.00 | 0 | 100.0 | **A** |
| 2.6c | IBRAE | 520 | 4.66–35.00 | 1.00 | 0 | 100.0 | **A** |
| 2.6c | IRSN | 529 | 2.90–34.94 | 0.97 | 0 | 100.0 | **A** |
| 2.6c | JAEA | 537 | 3.60–34.94 | 1.00 | 0 | 100.0 | **A** |
| 2.6c | KAERI | 526 | 4.07–34.94 | 1.00 | 0 | 100.0 | **A** |
| 2.6c | NRA | 389 | 4.19–34.94 | 0.75 | 0 | 100.0 | **B** |
| 2.6c | SNL | 529 | 3.84–34.94 | 1.00 | 0 | 100.0 | **A** |
| 2.6c | VTT | 579 | 0.86–34.94 | 1.00 | 0 | 100.0 | **A** |
| 2.6d | CIEMAT | 393 | 4.20–34.93 | 0.74 | 0 | 100.0 | **B** |
| 2.6d | IAE | 515 | 5.06–34.93 | 0.99 | 0 | 100.0 | **A** |
| 2.6d | IBRAE | 516 | 5.35–34.98 | 1.00 | 0 | 100.0 | **A** |
| 2.6d | IRSN | 540 | 2.99–34.93 | 0.98 | 0 | 100.0 | **A** |
| 2.6d | JAEA | 415 | 3.62–34.93 | 0.76 | 0 | 100.0 | **B** |
| 2.6d | KAERI | 521 | 4.08–34.93 | 0.99 | 0 | 100.0 | **A** |
| 2.6d | NRA | 388 | 3.91–34.93 | 0.73 | 0 | 100.0 | **B** |
| 2.6d | SNL | 409 | 3.68–34.93 | 0.76 | 0 | 100.0 | **B** |
| 2.6d | VTT | 457 | 0.86–34.93 | 0.78 | 0 | 100.0 | **B** |

**Code series: 28 A, 13 B, 11 C** on Figs 2.1, 2.2, 2.3b and 2.5; **30 A, 9 B,
6 C** on Figs 2.3a and 2.6 (Fig 2.6c is cleanest, 8 of 9 A; 2.6a has three C,
CIEMAT, JAEA and KAERI, hidden at ≈0.06 and ≈0 under later curves);
**58 A, 22 B, 17 C** in all. Fig 2.2 is the cleanest (8 of 9 A). Fig
2.1a is the worst (5 of 9 C), because its 7.4–7.7 MPa plateau is a band of
nine overlapping, noisy curves, and only the one drawn last survives in any
column.

---

## Limitations

1. **Hidden curve segments are absent, not zero.** A column-wise trace sees
   only the curve drawn on top. On Fig 2.5 the IAE and IBRAE curves at
   fraction 1.0 are under the curves drawn later, and on Fig 2.1a the early
   plateau is mostly VTT and SNL. Take the absence of a point as "unknown".
2. **Values on the axes are lost.** The left and bottom frame edges are on the
   axis lines, and the tracer's 5 px inset excludes them. A zero baseline,
   such as H2 = 0 or release = 0 before onset, is therefore not in the data
   on Figs 2.1, 2.2 and 2.5, and the first ~0.07 h of each panel is missing.
   Fig 2.3b keeps 0 kg/s, at the cost of SNL.
3. **Spikes one column wide are clipped.** KAERI's 8.3 MPa spike at 3.2 h on
   Fig 2.1a and IBRAE's 6.6 kg/s spike at ~255 h on Fig 2.3b are traced to
   ~7.8 MPa and 3.6 kg/s. A trace gives one value per column, so a near-vertical
   excursion contributes only its width.
4. **Two blend artefacts remain, on purpose.** (a) Fig 2.1b NRA at ~12.5 h
   jumps from 0.67 to 0.83 MPa for one column: there, the anti-aliased blend
   of the adjacent IAE blue and IRSN magenta lines is nearest NRA purple. The
   0.827 MPa "NRA peak" that `checks_unit1.py` prints comes from this blend;
   the figure's NRA peak is ~0.80 MPa near 15.3 h. (b) Fig 2.1b IBRAE dips to
   ~0.35 MPa at 6.1 h, where SNL's black vertical spike runs alongside. Both
   are counted in `SPK`. They were **not** smoothed away, because a filter
   that removes them also removes the genuine spikes.
5. **The drywell measurements are an envelope, not single readings.** From
   ~13 h on the markers overlap every ~7 px and are crossed by every curve.
   In the 16–23 h region two rows are visible, an upper one at ~0.76–0.77 MPa
   and a lower one at ~0.74 MPa. Whether that is two instruments or a
   scattered single series cannot be told from the raster. The marker-centre
   convention (bounding box) is also an assumption: if BSAF's markers are
   centred on the triangle centroid instead, every 2.1b measured value is
   **0.007 MPa lower** (H/6 = 5.7 px). That is a systematic uncertainty, and
   was not applied.
6. **Fig 2.3b's axis starts at 23.85 h**, not 0, by extrapolating the ticks to
   the y axis. VTT's 1.9 kg/s spike at the very start sits under the frame and
   is lost.
7. **Nothing was rescaled or tuned to a check.** Where a check disagrees
   (KAERI's MSL failure time, NRA's injection, the brief's expected drywell
   values), the disagreement is reported and the data are left as traced.
8. **Not reviewed by a human**, and the grades are from automated metrics, not
   judgment.

Added 2026-10-29 with Figs 2.3a and 2.6:

9. **The legend hides data on Figs 2.6c and 2.6d.** Unlike every earlier
   panel, curves run behind the legend box there, from **26.8 to 34.4 h**: NRA
   on 2.6c (0.41–0.44), and CIEMAT, JAEA, VTT, NRA and SNL on 2.6d (0.41–0.62).
   Those series have a gap there and resume at 34.4–35 h; nothing was
   interpolated across it.
10. **SNL at zero is lost on Figs 2.6a and 2.6b**, as on Fig 2.3b: the black
    axis line cannot be told from the black curve, so for SNL the frame stays
    on the axis. SNL's I trace ends at **0.019 at 111.9 h**, where its curve
    reaches the axis (Fig 2.6b); its Cs trace stays above the axis (0.061 at
    500 h). Other series at zero on Fig 2.6 are traced on the axis (KAERI,
    NRA, VTT; IBRAE throughout on 2.6c, d), but where several share the axis
    only the one drawn last survives in a column.
11. **IBRAE on Figs 2.6a and 2.6b runs along the top of the axis** (0.700 and
    0.552). The axis limits are 0.70 and 0.55; if IBRAE's deposit is higher,
    MATLAB clipped it and the figure cannot say.
12. **Fig 2.6b is a soft raster**, traced with Fig 2.6a's legend colours and a
    90-unit cap (Method). One crossing artefact remains and is kept: **NRA at
    246.4–250.7 h reads 0.104–0.107**, which is IAE's line (a blurred IAE edge
    nearest NRA purple), while NRA is at ≈0.005 there. It is 6 points long, so
    `SPK` (1–3 points) does not count it; a 4–12-point scan of every new series
    found no other. Its time axis is also the coarsest, 0.42 h per pixel.
13. **Fig 2.3a's measured data are an envelope**, as on Fig 2.1b, with 7.1 h
    bins and a `lower` time uncertainty of up to ±7.3 h (Method). The cluster
    at 20–29 h is a vertical stack across the S/C vent at ≈24 h (centres
    0.51–0.76 MPa within a few hours), partly left of the y axis; its upper and
    lower points bracket the fall but do not time it. The bounding-box centre
    convention again applies: a centroid convention would put every value
    **0.007 MPa lower** (H/6 = 5.8 px).

## Reproducing

This needs the private PDF, ImageMagick, `pdfimages` and `kovan-cli` (built
with `cargo build --release -p kovan`). Run Python as `python3 -I`, with the
images in their own directory:

```bash
pdfimages -png -f 19 -l 24 -p <report.pdf> img/u1     # PDF pages 19-24 = printed 17-22
python3 -I tools/axes.py img/u1-019-000.png 0,5,10,15,20 0,1,2,3,4,8 x0 y0   # per panel; labels in this README
python3 -I tools/run_unit1.py tools/curves-unit1.tsv img work ./target/release/kovan-cli
WIN=0.26 python3 -I tools/despeckle.py work/raw/fig2.[125]*.csv
WIN=4.0  python3 -I tools/despeckle.py work/raw/fig2.3b*.csv
python3 -I tools/measure_unit1.py img work
python3 -I tools/quality_unit1.py tools/curves-unit1.tsv img work/clean
python3 -I tools/checks_unit1.py work/clean
python3 -I tools/finalise_unit1.py tools/curves-unit1.tsv work/clean <this folder>
```

Figs 2.3a and 2.6 (2026-10-29), from the same `img/` folder; `FIGS` restricts
`run_unit1.py`, `measure_unit1.py` and `quality_unit1.py` to the new figures,
and `finalise_unit1.py` is then run over the union of both `clean/` folders,
which rewrites the earlier files byte for byte:

```bash
DARK=180 python3 -I tools/axes.py img/u1-022-003.png 50,100,150,200,250,300,350,400,450,500 0,0.2,0.4,0.8,1.0 y0
FRAC=0.2 python3 -I tools/grid_lin.py img/u1-024-002.png 185 1350 50 1030 100,200,300,-,400,- 0.2,0.3,-,-,0.4,0.5,0.6
FRAC=0.2 python3 -I tools/grid_lin.py img/u1-024-003.png 185 1350 50 1030 100,200,300,-,400,-,500 0.1,0.2,-,0.3,0.4,0.5,-
FRAC=0.2 python3 -I tools/grid_lin.py img/u1-024-004.png 185 1350 50 1030 5,10,15,20,25,-,30,- 0.1,0.2,-,0.3,0.4,0.5,0.6,0.7,-,-,0.8
FRAC=0.2 python3 -I tools/grid_lin.py img/u1-024-005.png 185 1350 50 1030 5,10,15,20,25,-,30 -,0.1,0.2,0.3,0.4,0.5,0.6,-
FIGS=2.3a,2.6a,2.6b,2.6c,2.6d python3 -I tools/run_unit1.py tools/curves-unit1.tsv img work2 ./target/release/kovan-cli
WIN=4.0  python3 -I tools/despeckle.py work2/raw/fig2.3a*.csv work2/raw/fig2.6[ab]*.csv
WIN=0.38 python3 -I tools/despeckle.py work2/raw/fig2.6[cd]*.csv
FIGS=2.3a python3 -I tools/measure_unit1.py img work2
FIGS=2.3a,2.6a,2.6b,2.6c,2.6d python3 -I tools/quality_unit1.py tools/curves-unit1.tsv img work2/clean
python3 -I tools/finalise_unit1.py tools/curves-unit1.tsv <work/clean + work2/clean> <this folder>
python3 -I tools/checks_unit1_late.py <this folder>
```

`calib.py` is copied unchanged from the Cs-137 folder, because the new tools
import it. `grid_lin.py` and `checks_unit1_late.py` were added on 2026-10-29;
`axes.py`, `run_unit1.py`, `measure_unit1.py`, `quality_unit1.py` and
`finalise_unit1.py` were extended then, with every change opt-in per figure. `swatch.py`, `isolate_all.py` and `despeckle.py` are changed
copies; the changes are marked in their comments, and the Cs-137 originals are
untouched.
