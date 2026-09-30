# TRISO particle failure in air: JAERI and KORA tests, and Nabielek's failure model (IAEA-TECDOC-978 §5.4)

Measured coated-particle failure under air at high temperature (JAERI,
unirradiated; KORA, irradiated), and the Nabielek air-ingress failure model
fitted to the KORA data. Needed by the oxidation-attack epic (#441/#443) and
the conservative bound (#435, `f_ox`).

**What kind of numbers these are:**
- Tables 5-6 and 5-7 are **experimental**.
- The Nabielek model is an **empirical fit** to Table 5-7 (Weibull-like in
  time, Arrhenius in temperature). It is *not* first principles.

---

## Provenance

| Field | Value |
|---|---|
| Source | IAEA-TECDOC-978, *Fuel performance and fission product behaviour in gas cooled reactors*, IAEA, Vienna, 1997 |
| Sections | §5.4 (printed pp.243–251): §5.4.1 (JAERI), §5.4.2.2 (KORA), §5.4.2.3 (model); Tables 5-6, 5-7; Eqs. 5-9 to 5-11; Figs. 5-18 to 5-24 |
| Copy held | the maintainer's private repository, `literature/proprietary/2002iaeatecdoc978.pdf` (533 pp.; the tables are on PDF pages 252 and 254) |
| Access tier | **Proprietary**, following the workspace's treatment of TECDOC-1382 (reclassified 2026-09-22 for want of a verified reuse licence). Facts only |
| Processing | **Tables 5-6 and 5-7 and the model constants were transcribed by an AI agent from rendered page images** (PDF pages 252, 254, 255, 259 at 110–130 dpi), because the text layer garbles them, 2026-09-30. **Not reviewed by a human.** ~~No figure was digitised.~~ **UPDATED 2026-09-30:** Figs. 5-18, 5-21 and 5-23 were **digitised by the maintainer by hand in Kovan** (see §4). |

---

## 1. Table 5-6: JAERI, unirradiated fuel in air (printed p.245)

| # | Sample | Particles tested | T (°C) | Time (h) | Failed | SiC failure fraction (as printed) | failed ÷ tested |
|---|---|---|---|---|---|---|---|
| 1 | Coated particles | 3151 | 900 | 40 | 1 | 2.3×10⁻⁴ | 3.2×10⁻⁴ |
| 2 | Coated particles | 3127 | 1000 | 40 | 0 | 5.1×10⁻⁶ | 0 |
| 3 | Coated particles | 3136 | 1200 | 40 | 0 | 1.0×10⁻⁶ | 0 |
| 4 | Coated particles | 3123 | 1300 | 600 | 2 | 5.4×10⁻⁴ | 6.4×10⁻⁴ |
| 5 | Coated particles | 3114 | 1400 | 40 | 1 | 3.2×10⁻⁵ | 3.2×10⁻⁴ |
| 6 | **Fuel compact** | 10461 | **900** | 54 | 13 | **1.2×10⁻³** | 1.24×10⁻³ |
| 7 | Fuel compact | 10599 | 1400 | 20 | 8 | 6.9×10⁻⁴ | 7.5×10⁻⁴ |

**The printed fraction is not failed ÷ tested in most rows.** Rows 2–3 print
non-zero fractions for zero failures, and row 5 differs by a factor of 10.
The printed column is presumably from a separate measurement of SiC failure,
such as burn-leach; the source does not say. **Quote the printed column as
the source's value, and state which column is used.**

**What the source says about it (§5.4.1):**
- SiC failure in compacts *"was highest for those fuel compacts heated at
  900 °C"*.
- The explanation: *"Burning the graphite matrix is exothermic so that the
  temperature at oxidizing local points might be far higher than 900 °C … the
  particle failure in the fuel compacts during burning might not be strongly
  dependent on the general temperature."*
- Fig. 5-18: a compact at 1400 °C lost weight to **42 %** in ≈ 20 h, as the
  carbonaceous material burned off, then stayed constant to 40 h with bare SiC
  particles.
- *"Once the exterior SiC surface was exposed, following the complete burning
  of the outer pyrocarbon coating, the fuel would not oxidize, while the SiC
  coating is intact."*

## 2. Table 5-7: KORA, irradiated UO₂ TRISO in air (printed p.247)

**HUMAN-CHECKED 2026-09-30.** The maintainer entered Table 5-7 by hand in Kovan,
eye-checked against the page (Kovan folder `papers/2002/2002iaeatecdoc978/`,
dataset `table-5-7`, PDF page 254). An AI agent compared it with the
transcription below: **all 7 rows and all 9 columns agree exactly.** The rest
of this file (Table 5-6, the model constants, the text facts) is still
AI-transcribed and not human-reviewed.

Air at 30 l/h, ≈ 0.13 MPa. The same data as Kugeler et al. 2017 Table 9
(Schenk 1995); see `kugeler2017-oxidising-accident-tests.md`.

| Fuel sample | Particles | Burnup (% FIMA) | Heat-up (h) | Max T (°C) | Time (h) | 1st failed after | Failed | Fraction failed |
|---|---|---|---|---|---|---|---|---|
| 92/29, 12 | 10 | 9.2 | 14 | 1400 | 400 | 397 h | 1 | 0.1 |
| 73/8, 11 | 10 | 4.7 | 15 | 1500 | 25 | 8 h | 10 | 1 |
| 92/29, 13 | 10 | 9.2 | 15 | 1500 | 25 | 3 h | 10 | 1 |
| 92/29, 11 | 10 | 9.2 | 28 | 1620 | 1 | at 1613 °C | 10 | 1 |
| AVR 89/12 | 16,400 | 9.4 | 13 | 1300 | 410 | 258 h | 4 | 2.4×10⁻⁴ |
| AVR 92/22 | 16,400 | 8.8 | 14 | 1400 | 140 | 1 h | 20 | 1.2×10⁻³ |
| AVR 89/14 | 16,400 | 9.0 | 14 | 1400 | 70 | 2 h | 12 | 7.3×10⁻⁴ |

**Text facts (§5.4.2.2):**
- At **1100 °C**, particle defects are not induced, while *"corrosion of the
  matrix graphite and the outer pyrocarbon coating of the particles proceeds
  rather rapidly"*.
- The graphite combustion completes after **≈ 100 h** at 30 l/h, and the
  particles were all still gastight.
- Fig. 5-19: H-3 and C-14 are released during graphite oxidation.

## 3. Nabielek's air-ingress particle-failure model (§5.4.2.3)

**Constant temperature (Eq. 5-9):**

```
F = 1 − exp{ −(k t)² },    k = k0 · exp(−Q / (R T))
```

where F is the fractional failure, t is in s, k and k0 are in s⁻¹, Q is in
J/mol, R = 8.314 J/(mol K), and T is in K.

**One mechanism.** Fitted to the 1300 and 1400 °C data, it **under-predicts
1500 °C by a factor of about 10**.

**Two mechanisms (the "2-mec model", ref. [37]),** which *"predicts quite well
the measurements at 1300, 1400, and 1500 °C"*:

| Mechanism | Q (kJ/mol) | k0 (s⁻¹) |
|---|---|---|
| 1 | **1221** | **1.467×10³¹** |
| 2 | **555.5** | **2.508×10¹⁰** |

The two line segments are summed (§5.4.2.3: *"the sum of the continuous line
segments … yields a smooth curve"*), so `k = k1 + k2`.

**Varying temperature.** Replace `kt` by the action integral `∫ k(T(t′)) dt′`.
For the 2-mec model, the interval average is

```
<k> = (k2 T2 F2 − k1 T1 F1) / (T2 − T1),   Fi = 1 − xi · exp(xi) · E1(xi)   (Eqs. 5-10, 5-11)
```

with `xi = Qi / (R Ti)`, i = 1 or 2 (the interval end points), and `Ei(xi)`
the exponential integral (printed p.252).

**Ramp test (Fig. 5-24):** ten AVR 92/29 particles, room temperature → 1620 °C
over 28 h, then 1 h hold. The first failure was at 1613 °C (27.88 h), and all
10 had failed by the end. The single-Q profiles compared are A (912 kJ/mol,
the mean in Fig. 5-22), B (1220 kJ/mol, the upper value) and C (884 kJ/mol).
The fractional release is strongly sensitive to Q.

The source's own caveat (p.252): *"sufficient data to provide a reliable mean
and variance for Q must be obtained before an adequate test of the models can
be conducted. At present, the model version corresponding to profile C at
least overpredicts the datum at 27.88 h."*

**Hand check (agent, 2026-09-30, 2-mec constants above, constant temperature):**

| Case | Model k (s⁻¹) | Model F | Measured (Table 5-7) |
|---|---|---|---|
| 1300 °C, 410 h | 9.41×10⁻⁹ | **1.9×10⁻⁴** | 2.4×10⁻⁴ (AVR 89/12) |
| 1400 °C, 140 h | 2.25×10⁻⁷ | **1.3×10⁻²** | 1.2×10⁻³ (AVR 92/22): **model ≈ 10× high** |
| 1400 °C, 70 h | 2.25×10⁻⁷ | **3.2×10⁻³** | 7.3×10⁻⁴ (AVR 89/14): **model ≈ 4× high** |
| 1500 °C, 25 h | 1.68×10⁻⁵ | **0.90** | 1 (10-particle batches) |
| 906.5 °C, 72 h (HTR-10 air ingress, Gao & Shi) | 6.3×10⁻¹⁵ | **≈ 3×10⁻¹⁸** (`(kt)²`; `1 − exp` underflows in f64) | — |
| 1033 °C, 72 h (the ATWS variant) | 1.5×10⁻¹² | **1.6×10⁻¹³** | — |

So the source's *"predicts quite well"* holds at 1300 and 1500 °C, but the
2-mec constants **over-predict the 1400 °C spheres by 4–10×**. Whether the
source's Fig. 5-23 shows the same was not checked (the figure was not
digitised).

At HTR-10's air-ingress temperatures the fit gives **negligible** failure.
The **JAERI 900 °C compact result above (1.2×10⁻³) contradicts it**, through
local exothermic heating that a nominal-temperature model cannot see (#445).

## 4. Digitised figures (the maintainer, by hand, in Kovan, 2026-09-30)

**Where they are:** the maintainer's Kovan folder, `papers/2002/2002iaeatecdoc978/2002iaeatecdoc978.md`
(private repository, because the source is proprietary). Each dataset records the page, the figure
region, both axis calibrations (pixel ↔ value), the axis scales (Fig. 5-21 and Fig. 5-23 are
log-y, linear-x), the method (`manual_digitisation`) and the digitiser. That satisfies the
workspace's figure-provenance rule.

| Figure | PDF page | Series |
|---|---|---|
| 5-18, compact weight in air at 1400 °C | — | 6 data points (0.5–19.9 h) |
| 5-21, failure fraction in air | 256 | 1300 °C (5 points); 1500 °C (5); 1400 °C, two series (9, 8) |
| 5-23, failure fraction at constant temperature | 257 | measurements: 1300 °C AVR "89/19", 1400 °C AVR 92/8, 92/22 and the 10-particle batch, 1500 °C 73/8. Predictions: Nabielek 1300 and 1400 °C, 2-mec 1500 °C |

### Which points to use (maintainer, 2026-09-30: *"5-21 is very hard to read, those 1400 °C points. If in doubt, ignore points."*)
- **Fig. 5-21's two 1400 °C series are EXCLUDED.** On its 0–400 h axis they are squeezed into the first ≈ 35 h and are hard to read. Use **Fig. 5-23** for 1400 °C (AVR 92/8, 92/22), with Table 5-7 for the end-of-test values.
- **Fig. 5-21's fifth 1300 °C point (4.9 failures at 383 h) is EXCLUDED** as doubtful: Table 5-7 and Fig. 5-23 both show 4 failures. The first four points, and Fig. 5-21's 1500 °C series, are unaffected.
- Nothing downstream used the excluded points. The 96 h conclusion below rests on Fig. 5-23 and Table 5-7.

### Checks by an AI agent, 2026-09-30
- **1300 °C points sit on whole failure counts** (F × 16 400):
  - Fig. 5-23: 1.05, 2.08, 2.93, 4.10, i.e. 1, 2, 3, 4 failures, consistent with Table 5-7 (4 by 410 h).
  - Fig. 5-21: 0.98, 2.01, 2.99, 3.97 **and a fifth point, 4.90 at 383 h**.
  - ~~**Open:** either Fig. 5-21 shows one more failure … or that point is misplaced.~~ **Resolved 2026-09-30:** the point is excluded as doubtful (see above).
- **Fig. 5-18:** the plateau is 42.4 % at 19.9 h, matching the text's "42 %".
- **1400 °C, 96 h:** the measured spheres reach ≈ 9×10⁻⁴ by 18–34 h (AVR 92/22, 92/8) and 1.2×10⁻³ at 140 h (Table 5-7). Failure only grows with time, so **1.2×10⁻³ bounds a 96 h period**. This is used by `sembawang`'s `htr10_air_ingress_kora_bound` (#435).
- **Nabielek's fit against measurement, from the source's own figure (5-23):**
  - the 1400 °C prediction is 3.7×10⁻³ at 68.6 h and 1.5×10⁻² at 133.8 h;
  - measured, 1.2×10⁻³ at 140 h: **≈ 12× over-prediction**.
  
  The agent's 2-mec calculation (§3) reproduces the figure's 1400 °C curve to within 17–21 % (0.79–0.83 of it), and its 1300 °C curve to 0.55. At 1500 °C it matches to 0.67–1.32. The figure's 1300 and 1400 °C "Nabielek" curves are probably the single-mechanism fit; that was not confirmed.
- **Source inconsistencies (not digitisation errors):**
  - Fig. 5-23 labels the 1300 °C sphere **AVR 89/19**, where Table 5-7 says **89/12**.
  - **AVR 92/8** (Figs. 5-19 and 5-23; 1400 °C, 9 % FIMA) does not appear in Table 5-7.
