# HTR-10: accident-analysis results (Gao & Shi 2002, Table 5 and §5.3–5.4)

The maximum values from HTR-10's accident analyses: fuel temperature, power,
pressure, graphite corrosion and particle exposure, per initiating event.
Needed by the conservative bound (#435) and the oxidation-attack epic
(#441/#442).

**What kind of numbers these are: CALCULATED, not measured.**
- They come from HTRSIMU, built on Jülich's **THERMIX/REACT**: 2-D transient
  conduction, quasi-steady flow, point kinetics, and REACT graphite corrosion
  (C+O₂, C+H₂O, C+CO₂, C+2H₂).
- The models were verified against AVR and KFA experiments (Kindt & Haque
  1992), but these HTR-10 accidents never happened.
- A comparison against these numbers is **code-to-code**, not validation.

---

## Provenance

| Field | Value |
|---|---|
| Source | Gao Zuying and Shi Lei, *Thermal hydraulic transient analysis of the HTR-10*, Nuclear Engineering and Design **218** (2002) 65–80, doi:10.1016/S0029-5493(02)00199-1 |
| Not the same paper as | the catalogued `gao2002htr10th` (NED 218, 51–64), the steady-state companion |
| Sections | Table 5 (p.73), §5.3.2 (p.78), §5.4 (pp.78–79), §1 (p.66) |
| Copy held | the maintainer's private repository, `literature/proprietary/zuying2002thermal.pdf` |
| Access tier | **Proprietary** (© 2002 Elsevier). Facts only |
| Processing | Table 5 **transcribed by an AI agent from the rendered page image** (PDF page 9, 120 dpi) and cross-checked against the text layer, 2026-09-30. §5 text from the text layer. **Not reviewed by a human.** |

---

## 1. Table 5: main results (maximum values)

Blank cells are blank in the source.

| Initial event | Power (MW) | Pressure (MPa) | Fuel T (°C) | Graphite corrosion (kg) | Exposure ratio of the first coated particles (%) |
|---|---|---|---|---|---|
| Inadvertent main helium blower acceleration | 6.049 | 3.13 | 899 | | |
| Failure of blower shut-off | 0.552 | 3.867 | 1033 | | |
| Loss of external power supply | 10.5 | 3.18 | 1033 | | |
| Loss of normal water flow | 10.5 | 3.4 | 1033 | | |
| Loss of normal water flow and the blower baffle fails to close | 10.5 | 3.5 | 1033 | | |
| Control rod withdrawal at power operation mode | 23.6 | 3.0 | 1170 | | |
| Control rod withdrawal at sub-critical or startup operating mode | 1.5 | 3.0 | 63.9 | | |
| Pebble bed densification caused by earthquake | 15.2 | 3.0 | 1038 | | |
| Control rod withdrawal at earthquake | 28.5 | 3.0 | 1209.9 | | |
| **Water ingress into primary loop system** | 11.2 | 3.15 | **1036** | | |
| All control rods drawn out uncontrolled at cold startup | 0.714 | 0.1 | 51.3 | | |
| Large pipe (DN65) rupture between pressure vessel and isolated valve of primary loop | 10.5 | 3.0 | 1033 | | |
| Heat exchanger tube rupture | | 3.43 | | 4.88 | |
| Rupture of heat exchanger tubes | | 3.5 | | 4.88 | |
| **Pressure vessel of hot gas duct rupture and reactor cavity cooling system fails to work (the third process)** | | 0.1 | **906.5** | **319.2** | **2.4** |
| Loss of external power (ATWS) | 10.5 | 3.18 | 1033 | | |
| Loss of normal water supply ATWS and the blower baffle fails to close | 10.5 | 3.5 | 1033 | | |
| Control rods withdrawal (ATWS) | 23.6 | 3.0 | 1172 | | |
| Control rod withdrawal ATWS and the blower baffle fails to close | | 3.83 | 1099 | | |
| Control rod withdrawal ATWS and reactor system loss of pressure | 10.5 | 0.1 | 1109 | | |
| **Pressure vessel of hot gas duct rupture ATWS and reactor cavity cooling system fails to work** | 10.5 | 0.1 | **1033** | | |

**Note, for the air-ingress bound:** the **ATWS** variant of the hot-gas-duct
rupture reaches **1033 °C**, which is higher than the non-ATWS "third process"
at 906.5 °C. Table 5 gives no corrosion or exposure for the ATWS variant.

## 2. Air ingress: hot-gas-duct rupture (§5.3.2, p.78)

| Fact | Value |
|---|---|
| Break | internal and external hot-gas-duct tubes break simultaneously |
| Phases | depressurisation, natural convection, graphite corrosion |
| Cavity rupture disc | breaks at cavity pressure > **0.11 MPa**; discharge to the environment **without filtration** |
| Ventilation after pressure equilibrium | filtered; ventilating flow **100 % d⁻¹ for the first 3 days**, then the air source is conservatively assumed cut off and the cavity sealed (e.g. by foam) |
| Maximum fuel temperature, third process | *"remains below"* **906.5 °C** |
| Graphite corrosion | **319.2 kg** |
| Exposure | *"After 3 days into the accident, about 2.4% of the first coated particles will be exposed"* |
| Particle integrity | *"The integrity of the fuel particles and the ability of retaining fission product are kept well."* So **exposed ≠ failed**. |

**Not given anywhere in the paper:** the reactor-cavity volume, so no absolute
O₂ supply (#420). The definition of "first coated particles" is also absent.

## 3. Water ingress: SG leakage (§5.4, pp.78–79)

| Fact | Value |
|---|---|
| Scenario | two SG tubes rupture and the secondary relief system fails (a BDBA); initial power 105 % |
| Moisture detected | ≈ 15 s |
| Scram (reflector rods), blower off | at 37.5 s (primary-pressure sliding rate 0.031 MPa min⁻¹), blower 1 s later |
| Initial discharge from the rupture | ≈ **1.58 m³ s⁻¹** |
| Total water ingress | **129.9 kg** |
| Reactivity from moderation | ≈ **8.5×10⁻⁴ Δk/k** |
| Primary safety valve | opens ≈ 3 h (3.5 MPa), closes at 3.07 h (< 2.9 MPa) |
| Maximum fuel temperature | **1036.2 °C** (Table 5: 1036) |
| Maximum reactor power | **14.2 MW** in the text. **Table 5 gives 11.2 MW for the same event**; the source is inconsistent, and which is right is not stated |
| Graphite corrosion | *"remains less than"* **4.88 kg** |
| Particle exposure | *"the exposure of fuel particles will not occur"* |
| H₂ and CO in the primary loop | 0.64 % each |

## 4. Fuel-temperature limit used (§1, p.66)

The coating was *"experimentally proven to keep its integrity and retain the
radioactive fission products effectively up to 1250 °C"* during the first
phase of the project, so the **accident limit is set to 1230 °C**. This is
lower than the HTR-Module's 1600 °C.
