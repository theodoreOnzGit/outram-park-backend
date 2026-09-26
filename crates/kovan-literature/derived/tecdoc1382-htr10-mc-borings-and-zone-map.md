# HTR-10: the benchmark's Monte Carlo borings, density corrections and R-Z zone map

Facts extracted from **IAEA-TECDOC-1382 part 2, § 4.1.2**, for building the
reflector of `crates/nee_soon/src/htr10_rmc/` as explicit 3-D geometry. Every
number below is transcribed from the source. This record summarises and
tabulates; it does not reproduce the source's text.

**All lengths are in the units the source uses:** mm for the boring list, cm
for the zone map. They are not converted here, so they can be compared
byte-for-byte against the document.

---

## Provenance

| Field | Value |
|---|---|
| Source | IAEA (2003), *Evaluation of high temperature gas cooled reactor performance: Benchmark analysis related to initial testing of the HTTR and HTR-10*, IAEA-TECDOC-1382, part 2 |
| Pages | printed pp. 234 (Fig. 4.7), 239 (reactor-core parameters), 240 (Table 4-3), 241 (Fig. 4.10, boring 1), **242 (borings 2-5, corrections, B1-B3)** |
| Access tier | **Proprietary** in this workspace (reclassified 2026-09-22, `CATALOGUE.md`). The PDF lives in the maintainer's private literature repository, never here. Facts only are recorded. |
| Read by | **An AI agent (Claude), 2026-09-25, unreviewed by a human.** |
| Processing | See "How each item was read" below. No digitiser was used. Nothing was measured off a plot: every number is a printed label or a printed value. |

### How each item was read — read this before trusting any of it

- **Printed p. 242 has NO text layer.** It is a scanned image in the PDF
  (PDF page 16 of the part-2 file). Its contents are missing from the
  catalogued Markdown conversion, which runs straight from boring 1 on p. 241
  to § 4.1.2.4 on p. 243. That is why borings 2-5, the density corrections
  and the B1-B3 definitions never reached this workspace before. They were
  read by eye from the page rendered at 130 dpi (`pdftoppm -r 130`). The page
  is typeset text, so the reading is an OCR-by-eye of printed digits, not a
  figure reading.
- **Fig. 4.10 (the zone map)** is a raster. It was read from the page rendered
  at 400 dpi. Zone numbers and boundary coordinates are printed labels. The
  figure is **not to scale**: the label for r = 70.75 sits nearer 76 cm on a
  linear reading. So only the labelled coordinates are used, never a
  position measured off the drawing.
- **Fig. 4.7** (the horizontal cross-section) is a 416 x 233 px embedded raster,
  about 2 cm per pixel across the core. Only its printed dimensions (the KLAK
  slot) are used. **Channel azimuths cannot be read from it**, and none are
  recorded here.

### Cross-checks that make the reading trustworthy

1. **The zone map reproduces the maintainer's own reading of Terry (2005)
   Fig. 2** (`terry2005-htr10-rz-zone-geometry.md`). All 14 zones of the two
   bottom layers agree zone-for-zone and span-for-span. All ten radial
   boundaries agree, and 16 of the 17 axial ones do.
2. **The 17th axial boundary resolves that record's "one unreconciled value".**
   TECDOC Fig. 4.10 draws the pebble top at **171.698** cm, while Terry Fig. 2
   has **228.758**. 351.818 − 171.698 = **180.12 cm**, which is the **full
   core**: a 180 cm cylinder plus the 36.946 cm conus frustum
   (25 → 90 cm radius) holds 4.58 + 0.42 = **5.00 m³**, TECDOC's stated full-core
   volume. 351.818 − 228.758 = 123.06 cm is the critical loading. The two
   figures draw different loadings, so the values are not in conflict.
3. **Every boring reproduces Table 4-3's homogenised densities** (next section
   but one). That check would fail if a channel count, a diameter, a radial
   position, an axial extent or the KLAK slot area had been misread.

---

## 1. Reactor-core parameters (p. 239)

| Quantity | Value |
|---|---|
| Density of reflector graphite | 1.76 g/cm³ |
| Equivalent natural boron impurity in reflector graphite | 4.8366 ppm |
| Density of boronated carbon brick, B4C included | 1.59 g/cm³ |
| Weight ratio of B4C in boronated carbon brick | 5 % |

Zone 22 **is** this graphite, solid. The table states 8.82418e-2 C and
4.73769e-7 natural B. 1.76 g/cm³ gives 8.8243e-2, and 4.8366 ppm gives
4.742e-7, a 0.1 % difference. Zone 17 **is** the boronated brick, solid. It
states 7.65984e-2 C and 3.46349e-3 B, against 7.660e-2 and 3.466e-3 computed
from the parameters above.

## 2. Borings for a Monte Carlo model (pp. 241-242)

| # | Boring | Count | Diameter / radial centre [mm] | z extent [mm, from model top] | Notes |
|--:|---|--:|---|---|---|
| 1 | Helium flow channel | 20 | 80 / 1446 | 1050 – 6100 | |
| 2 | Control-rod channel | 10 | 130 / 1021 | 0 – 4500 | |
| 2 | Irradiation channel | 3 | 130 / 1021 | 0 – 4500 | same size and radius as the rod channel (also Fig. 4.7) |
| 3 | Small-absorber-ball (KLAK) channel | 7 | round, 60 / 986 | 0 – 1300 and 3887.64 – 6100 | |
| 3 | KLAK channel, core height | 7 | cross-section per Fig. 4.7 | 1300 – 3887.64 | the slot below |
| 4 | Hot gas duct | 1 | 300 diameter; axis at z = 4800 | R = 900 – 1900 | horizontal |
| 5 | Fuel discharge tube | 1 | 0 < R < 250 | 3887.64 < z < 6100 | |

**KLAK slot (Fig. 4.7):** a straight section of 100 mm between the centres of
two R30 semicircular ends, so 60 mm wide and 160 mm long overall. Area
= π·3² + 6·10 = **88.27 cm²**. Fig. 4.7 also states the radial coordinate
of the channel centre (986 mm). **The orientation is not stated.** It is
fixed by consistency: a tangential slot spans r 95.6-101.6 cm, inside the
rod-boring band, whereas a radial one would reach r 90.6 cm, into the band
whose zones (19-26, 80) carry no boring void.

**Azimuthal positions of every channel are NOT given** in the text, and
Fig. 4.7 cannot resolve them.

## 3. Density corrections when the borings are modelled (p. 242)

When the borings above are modelled explicitly, the source replaces the
homogenised Table 4-3 densities as follows:

| Zones | Take the densities of |
|---|---|
| 23, 25-26, 28, 30-41, 43-45, 49-50, 52-54, 58-59, 61-63, 66-67, 69-71, 80, 82 | zone 22 |
| 27, 46, 55, 64, 72, 74-79 | zone 17 |
| 47, 56, 65, 73 | zone 18 |
| 29, 42 | own value × **1.29978** |
| 60 | own value × **1.16051** |
| 6 | filled with graphite balls |

Zones not listed keep their Table 4-3 value: 0-4, 7-16, 17-22, 24, 48, 51,
57, 68 and 81 (5 is the void cavity).

### The corrections are exactly the boring void fractions

A correction factor f means the zone is (1 − 1/f) void. Areas of the rod
band (r 95.6-108.6 cm, 8339.6 cm²) and the coolant band (r 140.6-148.6 cm,
7268.4 cm²):

| Zone(s) | Borings present | Void from geometry | Void from Table 4-3 |
|---|---|---|---|
| 27, 28, 30, 41, 82 | 13 × Ø130 + 7 × Ø60 round | (1725.5 + 197.9) / 8339.6 = **23.06 %** | 1 − 0.0678899/0.0882418 = 23.06 %; 1/1.29978 → 23.06 % |
| 31-40 | 13 × Ø130 + 7 KLAK slots | (1725.5 + 7 × 88.27) / 8339.6 = **28.10 %** | 1 − 0.0634459/0.0882418 = 28.10 % |
| 43, 45 / 46 / 47 | 7 × Ø60 round | 197.9 / 8339.6 = **2.373 %** | 2.373 % (vs 22 / 17 / 18) |
| 58, 59, 61, 63 / 64 / 65 | 20 × Ø80 | 1005.3 / 7268.4 = **13.83 %** | 1 − 1/1.16051 = 13.83 % (vs 22 / 17 / 18) |
| 26, 44, 53, 62, 70, 77 | hot gas duct (+ the above) | 4.04 / 3.67+2.37 / 3.01 / 2.59+13.83 / 2.37 / 2.10 % | the same, each to < 0.01 % |

## 4. R-Z zone map (Fig. 4.10)

z is measured **downward** from the model top (0) to the bottom (610). The
pebble top is loading-dependent (171.698 in the figure, 228.758 at the
critical loading). Zones 31-40 are ten axial subdivisions of one box whose
internal boundaries are not labelled; all ten share one density.

| Zone | r [cm] | z [cm] |
|--:|---|---|
| 1 | 0 – 90 | 0 – 40 |
| 2 | 0 – 90 | 40 – 95 |
| 3 | 0 – 90 | 95 – 105 |
| 4 | 0 – 90 | 105 – 130 |
| 5 | 0 – 90 | 130 – pebble top (void) |
| — | 0 – 90 | pebble top – 351.818 (mixture balls) |
| — | cone | 351.818 – 388.764 (dummy balls above the slope) |
| 0 | cone – 90 | 351.818 – 388.764 |
| 6 | 0 – 25 | 388.764 – 495 |
| 7 | 0 – 25 | 495 – 540 |
| 81 | 0 – 25 | 540 – 610 |
| 8 | 25 – 90 | 388.764 – 402 |
| 9 | 25 – 90 | 402 – 430 |
| 10 | 25 – 41.75 | 430 – 450 |
| 11 | 41.75 – 90 | 430 – 450 |
| 12 | 25 – 41.75 | 450 – 510 |
| 13 | 41.75 – 90 | 450 – 465 |
| 14 | 41.75 – 70.75 | 465 – 495 |
| 15 | 70.75 – 90 | 465 – 495 |
| 16 | 41.75 – 90 | 495 – 510 |
| 17 | 25 – 95.6 | 510 – 540 |
| 18 | 25 – 95.6 | 540 – 610 |
| 19, 20, 21 | 90 – 95.6 | 0–40, 40–95, 95–105 |
| 22 | 90 – 95.6 | 105 – 388.764 |
| 23, 24, 25, 26, 80 | 90 – 95.6 | 388.764–430, 430–450, 450–465, 465–495, 495–510 |
| 27, 28, 29 | 95.6 – 108.6 | 0–40, 40–95, 95–105 |
| 82, 30 | 95.6 – 108.6 | 105–114.7, 114.7–130 |
| 31-40 | 95.6 – 108.6 | 130 – 388.764 |
| 41, 42, 43, 44, 45, 46, 47 | 95.6 – 108.6 | 388.764–430, 430–450, 450–465, 465–495, 495–510, 510–540, 540–610 |
| 74 | 108.6 – 167.793 | 0 – 40 |
| 66 | 108.6 – 148.6 and 148.6 – 167.793 | 40 – 95 and 40 – 388.764 (one L-shaped zone) |
| 48, 57 | 108.6–140.6, 140.6–148.6 | 95 – 105 |
| 49, 50, 51, 52, 53, 54, 55, 56 | 108.6 – 140.6 | 105–388.764, 388.764–430, 430–450, 450–465, 465–495, 495–510, 510–540, 540–610 |
| 58, 59, 60, 61, 62, 63, 64, 65 | 140.6 – 148.6 | the same eight spans |
| 67, 68, 69, 70, 71, 72, 73 | 148.6 – 167.793 | 388.764–430, 430–450, 450–465, 465–495, 495–510, 510–540, 540–610 |
| 75, 76, 77, 78, 79 | 167.793 – 190 | 0–40, 40–465, 465–495, 495–540, 540–610 |

**Zone 66's L-shape.** No line in Fig. 4.10 closes the space
r 108.6-148.6, z 40-95, and no label sits in it. It is therefore part of
zone 66. This does not affect a Monte Carlo model, because zone 66 takes
zone 22's density either way (§ 3).

## 5. Benchmark states (p. 242)

- **B1, initial criticality:** loading height for k_eff = 1.0, measured from the
  top of the conus, helium atmosphere, 20 °C, **no control rod inserted**.
- B2: full core (5 m³) at 20, 120 and 250 °C, no rod inserted.
- B3: worth of all ten rods fully inserted (B31) and of one (B32, the others
  withdrawn), full core, 20 °C.

**The state of the absorber balls is not stated** for any problem. The KLAK
system is the reserve shutdown system (pp. 236, 238), so its channels are
taken as empty. That is an assumption, and it is listed as an open item.

## Status

**Unreviewed.** Read by an AI agent. A human should re-read p. 242 and
Fig. 4.10 against this record before anything here is described as more than
tentative.
