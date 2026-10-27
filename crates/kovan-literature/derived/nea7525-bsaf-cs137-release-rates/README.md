# Fukushima Daiichi Cs-137 release-rate histories, digitised from BSAF Figures 5.1–5.4

**21 time series, 10 403 points**: each BSAF Phase II participant's predicted
Cs-137 release rate to the atmosphere, in Bq/hr, over the first 20 days, for
Fukushima Daiichi units 1, 2 and 3 and for the three-unit total.

These are the curves needed to drive a **time-dependent** atmospheric release.
The integral releases alone (Table 5.1, in
[`../nea7525-bsaf-fukushima-daiichi-bands.md`](../nea7525-bsaf-fukushima-daiichi-bands.md))
give the total but not the shape, and the shape is what a plume model consumes.

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). These are code
> predictions of a real accident, digitised from a summary figure. They are not
> measurements, not a licensing input, and not authoritative for any facility.

---

## Provenance

| Field | Value |
|---|---|
| Source | OECD Nuclear Energy Agency (2021), *Benchmark Study of the Accident at the Fukushima Daiichi Nuclear Power Plant (BSAF Project) — Phase II: Summary Report*, NEA No. 7525. Operating agent: Institute of Applied Energy (IAE), Japan |
| Figures | 5.1 (unit 1, p.44), 5.2 (unit 2, p.44), 5.3 (unit 3, p.44), 5.4 (three-unit total, p.45) |
| Copy held | the maintainer's private repository, `propreitrary-literature-theodore`, `reports/nea-7525-bsaf-fukushima-daiichi-summary-report.pdf` |
| Access tier | **Proprietary** (© OECD 2021). The PDF and the figure rasters stay private. Extracted coordinates are facts, published here with acknowledgement, as the OECD terms allow |
| Digitised | 2026-10-27, `kovan-cli digitise` (the workspace's own digitiser) with the pre- and post-processing in [`tools/`](tools) |
| Operator | AI agent, automatic. **Not reviewed by a human.** |

**Acknowledgement.** Source and copyright owner: OECD Nuclear Energy Agency,
*Benchmark Study of the Accident at the Fukushima Daiichi Nuclear Power Plant*,
NEA No. 7525, © OECD 2021.

---

## Files

- **`all-series.csv`** — everything in long format:
  `figure, unit, series, time_day, release_rate_bq_per_hr`. Start here.
- **`fig<N>-<series>.csv`** — one series each: `time_day, release_rate_bq_per_hr`.
- **`fig<N>-digitised.png`** — the digitised data **replotted on its own**, not
  overlaid on the source figure, so it can be published while the copyrighted
  figure stays private. Put it beside the printed figure to check by eye.
- **`tools/`** — the whole pipeline, so this is reproducible rather than
  asserted.

Rate is written to **4 significant figures** and time to 4 decimals. One pixel
is **1.9 % in rate** and **0.012 day in time**, so anything finer is false
precision; the tracer's raw float output was rounded away deliberately.

`fig5.2-iae-sampson.csv` **does not exist**: IAE predicted essentially no unit 2
release ("little release predicted from the reactor building due to modelling
deficit", Table 5.1 note), and the figure carries no visible curve. An empty
file would have looked like a failed extraction.

---

## Method

1. **Extract the figure at native resolution.** The figures are embedded JPEGs
   at 449 ppi (~1 900 × 1 200 px); `pdfimages` takes them directly rather than
   re-rendering the page, which would only upsample.
2. **Find the plot frame** — longest dark horizontal/vertical runs
   (`tools/probe.py`).
3. **Verify the calibration against the drawn gridlines** (`tools/calib.py`).
   This is a check, not an assumption: the axis is claimed to run 1e15 at the
   top frame to 1e7 at the bottom over 8 decades, so the gridlines must be
   evenly spaced. Measured spacing on Fig. 5.1 was **125.0–126.0 px** against
   **125.38 px** predicted, and the outer gridlines extrapolate onto the frame
   edges to within 1 px. The frame edges are the axis limits.
4. **Take each curve's colour from its own legend swatch** (`tools/swatch.py`)
   rather than guessing it.
5. **Mask the legend box** (painted white) so its swatch lines and grey label
   text are not traced as data. *This loses any curve passing under the legend*
   — see Limitations.
6. **Isolate each curve by nearest-colour classification**
   (`tools/isolate_all.py`), with a 30-unit cap. See "Why classification" below.
7. **Trace** with `kovan-cli digitise`, log y, linear x, explicit `--x-ref` /
   `--y-ref` from the verified pixels, `--step 2`, `--inset 8`.
8. **Drop isolated points** (`tools/despeckle.py`) — a real curve is a dense
   contiguous run; a lone point is a JPEG-blur false positive. 70 of 10 473
   points (0.7 %) removed.
9. **Verify** (below), then round and assemble (`tools/finalise.py`).

### Why nearest-colour classification, and not a per-colour threshold

Thresholding each curve colour independently **does not work here**, and the
failure is silent. SNL `(25,71,153)` and KAERI `(56,121,180)` are only ~65
apart in RGB; an anti-aliased SNL edge pixel falls inside KAERI's tolerance, and
the continuity tracer then walks down the wrong curve and stays there. The first
pass did exactly this: KAERI on Fig. 5.1 returned 1.6e13 at day 5.3 where the
figure shows ~4e14 — a factor of 25 — while reporting 763 points and a 100 %
"on-curve" self-check.

Classification fixes it: every ink pixel goes to whichever reference colour is
nearest **and to nothing else**, so two similar curves partition the pixels
instead of competing for them. The 30-unit cap then rejects ambiguous blends —
without it, a *lightened* anti-aliased edge of a dark curve lands nearer a
lighter reference, which made IRSN absorb CIEMAT's edges and over-read by 38×.

---

## Verification

### Independent check: integrate the curve, compare against Table 5.1

The curves come from pixels; Table 5.1's integral releases were transcribed by
hand from a different page. Nothing links them, so agreement is real evidence
that the trace found the right curve. Trapezoidal in rate against time, not
bridging gaps longer than 0.5 day.

**This is the check that caught the SNL/KAERI confusion** when the round-trip
check below did not.

### Round-trip check, and why it is weaker than it looks

`tools/roundtrip.py` reprojects every point to a pixel and confirms the source
image is that curve's colour there. All 22 series passed at 100 %. **This is
partly circular** — the tracer only accepted pixels within the same tolerance —
so it confirms the calibration round-trips and that nothing landed on the axis
or in blank space, but it cannot prove the right curve was chosen. Reported
because it was run, not because it is strong.

### Per-series quality

`INT` is the digitised integral in PBq, `TBL` the Table 5.1 value, `SPIKES` the
count of single-point out-and-back excursions over 0.6 decades (suspected
curve-crossing artefacts).

| Fig | Series | N | INT | TBL | Ratio | Spikes | Grade |
|---|---|---:|---:|---:|---:|---:|---|
| 5.1 | CIEMAT MELCOR | 744 | 5.60 | 5.60 | 1.00 | 0 | **A** |
| 5.1 | IAE SAMPSON | 159 | 4.26 | 4.40 | 0.97 | 1 | **A** |
| 5.1 | IRSN ASTEC | 120 | 0.04 | 0.10 | 0.43 | 0 | **C** |
| 5.1 | KAERI MELCOR | 761 | 14.87 | 18.90 | 0.79 | 0 | **B** |
| 5.1 | SNL MELCOR | 568 | 2.81 | 2.80 | 1.00 | 0 | **A** |
| 5.2 | CNL MELCOR | 628 | 8.18 | 8.10 | 1.01 | 0 | **A** |
| 5.2 | CRIEPI MAAP | 477 | 6.52 | 6.70 | 0.97 | 0 | **A** |
| 5.2 | IAE SAMPSON | 0 | 0.00 | 0.00 | — | 0 | **A** (no curve; table says 0.0) |
| 5.2 | IRSN ASTEC | 26 | 0.06 | 0.10 | 0.60 | 0 | **C** |
| 5.2 | KAERI MELCOR | 611 | 7.09 | 5.80 | 1.22 | 7 | **B** |
| 5.2 | SNL MELCOR | 515 | 8.51 | 12.30 | 0.69 | 0 | **C** |
| 5.3 | CRIEPI MAAP | 425 | 0.17 | 0.80 | 0.21 | 0 | **D** |
| 5.3 | IAE SAMPSON | 229 | 0.42 | 1.10 | 0.38 | 3 | **C** |
| 5.3 | IRSN ASTEC | 688 | 33.45 | 43.10 | 0.78 | 1 | **B** |
| 5.3 | KAERI MELCOR | 659 | 15.96 | 10.90 | 1.46 | 5 | **B** |
| 5.3 | PSI MELCOR | 13 | 0.02 | 0.20 | 0.10 | 0 | **D** |
| 5.3 | SNL MELCOR | 599 | 9.72 | 10.60 | 0.92 | 0 | **A** |
| 5.4 | GRS Backwards Calculation | 473 | 12.78 | 13.10 | 0.98 | 2 | **A** |
| 5.4 | IAE SAMPSON | 386 | 5.71 | 5.50 | 1.04 | 5 | **B** |
| 5.4 | IRSN ASTEC | 810 | 34.34 | 43.20 | 0.79 | 0 | **B** |
| 5.4 | KAERI MELCOR | 829 | 32.96 | 35.50 | 0.93 | 1 | **A** |
| 5.4 | SNL MELCOR | 683 | 27.72 | 25.70 | 1.08 | 1 | **A** |

**Median ratio 0.93; 17 of 21 within a factor of two; 9 within 5 %.**

Grades: **A** ratio 0.85–1.20 and ≤2 spikes · **B** 0.70–1.50 and ≤8 spikes ·
**C** 0.30–3.00, magnitude unreliable · **D** outside that, **do not use for
magnitude** — shape only.

---

## Limitations

1. **A column-wise trace cannot integrate a vertical spike.** One value per
   pixel column means a near-vertical excursion contributes its width, not its
   area. This is the cause of every **D** and most **C** grades: PSI on unit 3
   (0.10) is 13 points of pure vertical spikes, and IRSN on unit 1 (0.43) is
   dominated by narrow spikes at days 1.0–2.8. For these series the digitised
   integral is a **lower bound**.
2. **Curves drawn as disconnected fragments stay fragmented.** IAE on unit 3
   excludes 9.1 days of gaps because the figure itself only draws fragments;
   the missing release is not recoverable from this figure.
3. **Curves clipped at the frame are clipped in the data.** Several excursions
   run off the top (1e15) or bottom (1e7) of the axis. A value at a frame edge
   is a bound, not a reading. SNL on unit 2 loses 1.45 days this way (0.69).
4. **Residual crossing artefacts.** Where two similar curves cross, the tracer
   can still step onto the wrong one for a few columns, visible as a vertical
   out-and-back. KAERI on Fig. 5.1 shows these near days 9.3, 12.9, 14.0 and
   15.0 — the `SPIKES` column counts them. They were **not** smoothed away,
   because a filter that removes them also removes genuine spikes.
5. **Data under the legend box is lost** (step 5). The box occupies the lower
   right of each plot; any curve passing beneath it is absent, not zero.
6. **Not rescaled to Table 5.1.** It would have been easy to multiply each
   series by its ratio and report perfect agreement. That would convert the
   only independent check into an input and destroy it, which the workspace's
   "get the process right first" rule forbids. The ratios above are reported
   as the measured bias and left uncorrected.
7. **Not reviewed by a human**, and the grades are from automated metrics, not
   judgment.

## Reproducing

Needs the private PDF, ImageMagick, and `kovan-cli` built release.
`tools/curves.tsv` holds the frame pixels and legend colours for every series;
the commands are in the method above, in order.
