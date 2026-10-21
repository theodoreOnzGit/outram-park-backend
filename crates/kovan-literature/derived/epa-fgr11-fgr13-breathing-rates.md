# Breathing (inhalation) rates: EPA FGR-11 and FGR-13

The breathing rates available in the open corpus, for the optional dose stage
of the conservative bound (#437). Before 2026-09-30 the only rate in code was
pyDOSEIA's 8400 m³/y (`buangkok::pydoseia::dose::breathing_rate_m3_per_s`),
which is an upstream constant.

---

## Provenance

| Field | Value |
|---|---|
| Sources | **FGR-11**: Eckerman, Wolbarst & Richardson (1988), EPA-520/1-88-020 (`epa-fgr-11`). **FGR-13**: Eckerman, Leggett, Nelson, Puskin & Richardson (1999), EPA 402-R-99-001 (`epa-fgr-13`) |
| Copies | `reactor-literature/kovan-standard-open-corpus/epa/` |
| Access tier | **Open**, on EPA's copyright statement (non-commercial, scientific and educational use). This is the basis recorded in the corpus README |
| Processing | Read by an AI agent from the PDF text layer, 2026-09-30. FGR-13 Table 3.1 is transcribed from the text layer, and its columns were aligned by eye. **Not reviewed by a human.** |

---

## Values

| Rate | m³/s | Source | Basis |
|---|---|---|---|
| **0.020 m³/min** (= 1.2 m³/h) | **3.33×10⁻⁴** | FGR-11, derived-air-concentration section: *"based on a normal breathing rate B of 0.020 m³/min"* (the DAC = ALI / 2.4×10³ m³ relation) | occupational reference, from ICRP 30 |
| 22.2 m³/d, adult male (ages 20, 50, 75) | 2.57×10⁻⁴ | FGR-13 Table 3.1, from ICRP 66 | **24-h time-weighted average** of rest, light and heavy activity |
| 17.7 m³/d, adult female | 2.05×10⁻⁴ | FGR-13 Table 3.1 | the same |
| 8400 m³/y | 2.66×10⁻⁴ | pyDOSEIA (already in `buangkok`) | upstream constant |

FGR-13 Table 3.1, air (m³/d), all ages:

| Age (y) | 0 | 1 | 5 | 10 | 15 | 20 | 50 | 75 |
|---|---|---|---|---|---|---|---|---|
| Male | 2.9 | 5.2 | 8.8 | 15.3 | 20.1 | 22.2 | 22.2 | 22.2 |
| Female | 2.9 | 5.2 | 8.8 | 15.3 | 15.7 | 17.7 | 17.7 | 17.7 |

**FGR-13's own caution** (p.8): *"if the exposure scenario involves acute
inhalation of a radionuclide in a rapidly passing cloud, the average
inhalation rate in the exposed population during the exposure period may
differ from the 24-h average rate."* That is why the bound takes the larger
FGR-11 rate. A heavy-activity rate (ICRP 66) would be higher still, but it is
not in the corpus.
