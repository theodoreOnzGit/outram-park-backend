# TRISO fuel under steam and air ingress: German test results (Kugeler, Nabielek & Buckthorpe 2017, §4.3)

Measured behaviour of irradiated German LEU UO₂ TRISO fuel under steam
(water-ingress simulation) and air (air-ingress simulation), with the fuel
performance limits. Needed by the conservative bound (#435) and the
oxidation-attack epic (#441, #443, #444).

**What kind of numbers these are: EXPERIMENTAL** (KORA furnace, Jülich hot
cells; in-reactor and out-of-reactor steam tests).

---

## Provenance

| Field | Value |
|---|---|
| Source | Kugeler, K., Nabielek, H. and Buckthorpe, D. (2017). *The High Temperature Gas-cooled Reactor: Safety considerations of the (V)HTR-Modul.* EUR 28712 EN, JRC107642, doi:10.2760/270321 |
| Catalogue key | `kugeler2017vhtr` |
| Sections | §4.3 (report pp.46–48): §4.3.1 steam, §4.3.2 air, Table 9; §4.4 limits |
| Copy | `reactor-literature/theodore-open-corpus/jrc/kjna28712enn.pdf`; full text in `generated/markdown/open/vhtr-modul-safety-jrc.md` |
| Access tier | **Open**: reuse authorised with acknowledgement (EC Decision 2011/833/EU) |
| Processing | Read by an AI agent from the PDF text layer (`pdftotext -layout`), 2026-09-30. Table 9 cross-checked row by row against IAEA-TECDOC-978 Table 5-7 (read from the rendered page); every row agrees. **Not reviewed by a human.** |

---

## 1. Steam (water-ingress simulation), §4.3.1

| Finding | Value |
|---|---|
| Intact particles | *"Intact TRISO-coated particles are not affected by water vapour over a wide range of temperatures."* |
| Defective particles | steam *"can lead to the additional release of fission gases and iodine from defective (manufacture-induced) and failed (irradiation-induced) fuel particles"* |
| AVR 89/30 (irradiated sphere, 2 fabrication-defective particles), intermittent steam | **57 %** of the stored ⁸⁵Kr inventory released during the steam injection |
| Cracked irradiated kernels, repeated steam at **800 °C**, 5 % FIMA | **≈ 2 %** of ⁸⁵Kr released |
| The same, 9 % FIMA | **> 17 %** of ⁸⁵Kr released |

The steam effect is strongly burnup-dependent. HTR-10's 8.51 % FIMA is near
the upper value.

## 2. Air (air-ingress simulation), §4.3.2 and Table 9

**Text facts:**
- The prerequisite is a very large leak, e.g. rupture of the vessel
  connecting core and steam generator.
- Under extreme circumstances (open cell, open building), it takes **1 to 2
  days** of corrosion before the fuel-free zone is gone and the first
  particles are exposed.
- At **1100 °C**, complete oxidation of the graphite matrix of a sphere takes
  **70 to 100 h**, and *"the TRISO-coated UO₂ fuel particle remained intact
  over this period."*
- The first SiC damage is at **1300 °C**, after more than 10 days. The first
  failures come after 1–2 h at 1400 °C (possibly as-manufactured defects), and
  after 3–8 h at 1500 °C. At ≈ 1600 °C, particles fail at the end of heat-up.
- Air flow in the tests was 30 l/h.
- *"early assumptions predicted complete particle failure at 1 100 °C"*, which
  the tests refute.

**Table 9** (KORA, Schenk 1995): the **same data as TECDOC-978 Table 5-7**,
transcribed in `tecdoc978-air-oxidation-particle-failure.md` §2. Rows:
- 92/29.12: 1400 °C / 400 h → 0.1
- 73/8.11: 1500 °C / 25 h → 1.0
- 92/29.13: 1500 °C / 25 h → 1.0
- 92/29.11: 1620 °C / 1 h → 1.0
- AVR 89/12: 1300 °C / 410 h → **2.4×10⁻⁴**
- AVR 92/22: 1400 °C / 140 h → **1.2×10⁻³**
- AVR 89/14: 1400 °C / 70 h → **7.3×10⁻⁴**

## 3. Fuel performance limits, §4.4

German LEU UO₂ TRISO shows:
- near-complete retention of fission products in intact particles at
  < 1250 °C in normal operation, and at **≤ 1600 °C** in accidents;
- very low contamination in the outer coating and the matrix.

The measured free-uranium fractions are already in code:
`boon_lay::fuel_failure::htr10::qualification::FREE_URANIUM_FRACTIONS`.
