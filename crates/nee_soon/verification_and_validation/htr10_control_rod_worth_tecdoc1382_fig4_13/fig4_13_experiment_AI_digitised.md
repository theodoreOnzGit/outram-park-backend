# HTR-10 single control rod worth, experiment series: AI digitisation of IAEA-TECDOC-1382 Fig. 4.13

> **AI DIGITISATION.** This was produced automatically by a Claude Code agent on 2026-10-23, by colour segmentation of the figure's raster. It is **not** a human digitisation and **not** a kovan digitisation. Treat it as untrusted draft data until a human has reviewed it. The maintainer is supplying a kovan digitisation of the same series for comparison. Use for research, education and V&V only.

**Generated:** 2026-10-23 (UTC)

Data file: `fig4_13_experiment_AI_digitised.csv`. Extraction script: `fig4_13_ai_digitise.py`, run as `python3 -I fig4_13_ai_digitise.py <raster.jpg> <out.json> <out.csv>`. The source raster is not committed, because it is the IAEA's figure; extract it with `pdfimages -f 26 -l 26 -j` from part 2 of the TECDOC. The script writes both series, and only the experiment rows are kept here. The 2.7 cm point comes from the script's first pass (the `components` list in the JSON). It has 107 rows: 8 marker points and 99 trace points. Every row is tagged `digitiser = ai:claude-code`.

## Source

| field | value |
|---|---|
| document | IAEA-TECDOC-1382, *Evaluation of high temperature gas cooled reactor performance: Benchmark analysis related to initial testing of the HTTR and HTR-10* (IAEA, Vienna, 2003) |
| figure | Fig. 4.13, "Reactivity worth curve of one control rod", ~~printed p. 251~~ **printed p. 252** (part 2 PDF, page 26) |
| series | **"Experiment"** only (magenta squares; the S3 rod-worth measurement) |
| licence | openly published by the IAEA; no reuse licence was found in the document. Only digitised numbers are given here, with a citation. |
| source raster | the embedded **304 × 192 px JPEG** in the PDF, at its original resolution |

**Axes (TECDOC ~~pp. 251–252~~ printed pp. 252–253: §4.2.1.5 and §4.2.1.6):**
- **x:** the height of the rod lower end above z = 3942 mm, which is fully in for S3.
- **y:** reactivity worth in Δk/k, zero at lower end z = 1712 mm, i.e. x = 223 cm.
- The core was 17,000 balls with a loading height of 123.86 cm.

## Method

1. **Axis calibration.** Least-squares fits to all 30 x-ticks and 7 y-gridlines.
   - x: 1.2096 cm/px, RMS residual 0.39 cm.
   - y: 1.129e-4 Δk/k per px, RMS residual 3.7e-5.
2. **Segmentation.** The magenta mask is `R>170, G<120, R−G>90`, limited to the plot box with the legend excluded.
3. **Markers.** A 3×3 erosion removes the connecting line, and each remaining blob's centroid gives a point. One blob was two overlapping squares and was split by 2-means. One point, at 2.7 cm, is the centroid before erosion because the y axis clips it.
4. **Trace, 42–160 cm.** Here the squares overlap into one band, so the individual measurements **cannot be separated**. Each pixel column gives the mean row of the band. That is the drawn curve, not measured points.

## Uncertainty

- **±1 px resolution:** ±1.2 cm in elevation and ±1.1e-4 Δk/k in worth.
- **Worse in places:** the split pair and the clipped point at 2.7 cm could be ±2 px.
- **Trace bias, not measured:** the navy calculation line crosses the band at about 100–150 cm and can bias the trace there.

## Checks against the text

| check | text | AI digitised | difference |
|---|---|---|---|
| integral worth (x ≈ 0) | 1.4693 % (~~p. 251~~ printed p. 252, §4.2.1.5) | 1.462 % at 2.7 cm | −0.5 %, within 1 px |
| zero crossing | x = 223 cm by definition | 222.3 cm | 0.7 cm, within 1 px |

**A discrepancy in the source:** ~~p. 252~~ printed p. 253 (§4.2.1.6) gives the integral worth as **1.4369 %**. The digitised curve supports ~~p. 251's~~ printed p. 252's **1.4693 %**. This is not confirmed.

**CORRECTED 2026-10-23:** every printed-page cite here was one page early. The cites were checked against the page footers of part 2 of the PDF (`pdftotext`). PDF p. 26 is printed p. 252 (§4.2.1.5, the 1.4693 % and Fig. 4.13), and PDF p. 27 is printed p. 253 (§4.2.1.6, the 1.4369 %, INET's 1.448 % and the axis conventions). The control-rods agent found the error; it was gh:#1025 work.

## Marker points

| kind | elevation (cm) | worth (Δk/k) | note |
|---|---|---|---|
| marker_partial | 2.7 | 0.01462 | core eroded to 1 px; first-pass blob centroid; left edge clipped by y axis |
| marker | 19.9 | 0.01451 | marker body centroid after 3x3 erosion |
| marker | 146.4 | 0.00277 | two overlapping markers split by 2-means |
| marker | 154.6 | 0.00223 | two overlapping markers split by 2-means |
| marker | 167.4 | 0.00163 | marker body centroid after 3x3 erosion |
| marker | 179.8 | 0.00116 | marker body centroid after 3x3 erosion |
| marker | 199.9 | 0.00056 | marker body centroid after 3x3 erosion |
| marker | 222.3 | -0.00001 | marker body centroid after 3x3 erosion |

## Trace resampled every 10 cm

Interpolated from the CSV trace. These are points on the drawn curve, not measurements.

| elevation (cm) | 45 | 55 | 65 | 75 | 85 | 95 | 105 | 115 | 125 | 135 | 145 | 155 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| worth (Δk/k) | 0.01384 | 0.01320 | 0.01235 | 0.01130 | 0.01000 | 0.00869 | 0.00732 | 0.00603 | 0.00485 | 0.00373 | 0.00281 | 0.00212 |
