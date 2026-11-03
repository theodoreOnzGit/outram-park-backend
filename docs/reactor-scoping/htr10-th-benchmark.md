# HTR-10 thermal-hydraulics benchmark — cited input sheet

The inputs for an HTR-10 thermal-hydraulic model: the IAEA CRP-5 benchmark
(IAEA-TECDOC-1694, Chapter 2 and Section 7.1) and the published post-test
papers on the two safety demonstration tests. Every number carries a source,
a table or figure number, and a confidence level.

GitHub issue #1147 ("HTR-10 TH 1: cited input sheet"), under epic #1146.
Companion to [`htr10-plant-data.md`](./htr10-plant-data.md) (plant and piping),
[`htr10-neutronics.md`](./htr10-neutronics.md) and the scoping document
[`htr10.md`](./htr10.md). Machine-readable tables live in
[`htr10-th-data/`](./htr10-th-data/).

> **Intended use.** Education, research, capability building, and V&V only. This
> is an offline demonstration with no connection to any operational system. See
> `RESPONSIBLE_USE.md`.
>
> **Status.** Transcribed 2026-11-03 by an AI agent. It is draft material until a
> human reviews it. Every figure here is a *literature reading*. Nothing in this
> document has been checked against a running model, and no V&V result is
> claimed.

## 1. How to read this sheet

The convention is the one in
[`htr10-plant-data.md` Section 1](./htr10-plant-data.md#1-how-to-read-this-sheet):

| Level | Meaning |
|---|---|
| **Quoted** | The number is printed in the cited source at the cited table, figure or section. |
| **Derived** | Arithmetic done here on quoted numbers. The arithmetic is shown so it can be checked. |
| **Uncertain** | Read by eye from a figure, or internally inconsistent, or contradicted by another source. Do not use without re-reading the source. |

**Nothing here comes from general knowledge.** Where every source is silent,
the entry says **Unknown**. Physical constants used in a Derived check (the
helium gas constant) are named where they are used.

**Maintainer decisions applied (2026-11-03):**

- **D1.** Numbers from proprietary sources are recorded here with source and
  table or figure number. No text passages or figures are copied: numbers and
  short paraphrases only.
- **D4.** The transient tests use the **as-run** initial state. The full-power
  steady state uses the **benchmark specification**. Every conflicting
  alternative is kept as a **named input-sensitivity case** (Section 9).
- **D5.** The benchmark's helium correlations print pressure in Pa but need
  bar. The unit is fixed here and the fix is documented (Section 4.1). The
  bypass split was never measured. It is recorded as an assumption and a
  sensitivity input (Section 10).

**How the numbers were read.** Each PDF was converted with
`pdftotext -layout`. Every correlation table (TECDOC-1694 Tabs 2.8–2.13) and
every table where a column was blank or a sign was in doubt (Tabs 2.7, 7.1–7.3)
was checked against the page rendered with `pdftoppm`. The scratchpad extraction
report written earlier in this session was used only to find pages. **No number
was copied from it.** Where this pass disagrees with that report, this sheet
records what the source shows (Section 12).

**Coordinates.** Throughout, as in TECDOC-1694 Section 2.1.1.3: r is measured
from the core axis, z = 0 is the upper surface of the pebble bed, and z is
positive **downward**. Units are cm.

## 2. Source register and redistribution status

Redistribution status was read from each document's own front matter.

| Key | Document | Where it is | Redistribution |
|---|---|---|---|
| **[T1]** | IAEA, *Evaluation of High Temperature Gas Cooled Reactor Performance: Benchmark Analysis Related to the PBMR-400, PBMM, GT-MHR, HTR-10 and the ASTRA Critical Facility*, **IAEA-TECDOC-1694**, Vienna, 2013. ISBN 978-92-0-137610-7, ISSN 1011-4289. Chapter 2 (HTR-10), Section 7.1 (comparison). | Private repo, `iaea2013tecdoc1694.pdf` | **Not redistributable.** Its copyright notice says IAEA publications are protected under the Universal Copyright Convention and that permission is needed to use whole or parts of the texts. Numbers cited by table (D1). |
| **[H06]** | Hu S., Wang R., Gao Z., "Transient tests on blower trip and rod removal at the HTR-10", *Nucl. Eng. Des.* **236** (2006) 677–680. doi:10.1016/j.nucengdes.2005.11.015 | Private repo, `papers/hu2006-htr10-transient-tests.pdf` | **Not redistributable.** © 2006 Elsevier B.V., all rights reserved. |
| **[C09]** | Chen F., Dong Y., Zhang Z., Zheng Y., Shi L., Hu S., "Post-test analysis of helium circulator trip without scram at 3 MW power level on the HTR-10", *Nucl. Eng. Des.* **239** (2009) 1010–1018 | Private repo, `papers/chen2009-htr10-helium-circulator-trip.pdf` | **Not redistributable.** © 2009 Elsevier B.V., all rights reserved. |
| **[C14]** | Chen F. et al., "Progress of the HTR-10 measured data utilization", ICONE22-30088 (2014) | Private repo, `papers/chen2014-htr10-measured-data-utilization.pdf` | **Not redistributable.** Copyright © 2014 by ASME. |
| **[J09]** | Jun J. S., Lim H. S., Lee W. J., "The benchmark calculations of the GAMMA+ code with the HTR-10 safety demonstration experiments", *Nucl. Eng. Technol.* **41**(3) (2009) 307–318 | Private repo, `papers/jun2009-gamma-plus-htr10-benchmark.pdf` | **Not redistributable.** No copyright or licence statement found; held private by the owner's decision (private repo README). |
| **[G02]** | Gao Zuying, Shi Lei, "Thermal hydraulic transient analysis of the HTR-10", *Nucl. Eng. Des.* **218** (2002) **65–80** | Private repo, `zuying2002thermal.pdf` | **Not redistributable.** © 2002 Elsevier Science B.V. |
| **[K17]** | Kugeler K., Nabielek H., Buckthorpe D., *The High Temperature Gas-cooled Reactor: Safety considerations of the (V)HTR-Modul*, EUR 28712 EN, Publications Office of the EU, 2017. doi:10.2760/270321 | `crates/kovan-literature/reactor-literature/kovan-standard-open-corpus/eu-jrc/kjna28712enn.pdf` | **Open.** "Reuse is authorised provided the source is acknowledged" (Decision 2011/833/EU), printed on the copyright page. |
| **[P21]** | Putra F., "HTR-10 OTTO Cycle Depletion Simulation Using Discrete Element Method Coupled Monte Carlo", *J. Phys.: Conf. Ser.* **2048** (2021) 012031. doi:10.1088/1742-6596/2048/1/012031 | `.../theodore-open-corpus/cc-by/putra2021-htr10-otto-dem-monte-carlo.pdf` | **Open.** CC BY 3.0, printed on page 1. |
| **[S21]** | She D., Chen F., Xia B., Shi L., "Simulation of the HTR-10 Operation History With the PANGU Code", *Front. Energy Res.* **9** (2021) 704116. doi:10.3389/fenrg.2021.704116 | `.../theodore-open-corpus/cc-by/she2021pangu.pdf` | **Open.** CC BY, printed on the last page. |

**[G02] is not the plant-data sheet's [S5].** [S5] in `htr10-plant-data.md` is
Gao and Shi's *steady-state* paper (pp. 51–64). [G02] is their *transient*
paper in the same issue (pp. 65–80). Both are cited by key so they cannot be
confused.

**Not re-read in this pass:** Qin (JAERI-Conf 96-010, plant-data [S2]) and
IAEA-TECDOC-1382. No number in this sheet comes from them. Where this sheet
compares against [S2] or [S5], it cites the value recorded in
`htr10-plant-data.md`, says so, and does not re-check it.

**Not found in either repository:** the KTA 3102.1/.2/.3 rule texts
themselves; the INET commissioning reports cited by [H06] (Liang 2003; Hu
2003a, 2003b); and Chen et al. 2009, *JNST* 46(6), on the FPIC steady state,
cited by [C14].

## 3. Geometry for the R-Z model

### 3.1 Structure data ([T1] Tab 2.3, Tab 2.2)

All Quoted from [T1] Tab 2.3 unless marked otherwise.

| Quantity | Value |
|---|---|
| Core equivalent diameter | 180 cm |
| Core average height | 197 cm |
| Core volume | 5 m³ ([T1] Sec 2.1.1.5: includes the cone-shaped zone) |
| Volumetric filling fraction of balls | 0.61 (also [T1] Tab 2.2) |
| Empty cavity above the pebble bed | 41.7 cm |
| Fuel discharging tube diameter | 50 cm |
| Top reflector thickness | 90 cm |
| Top boronated carbon brick thickness | 40 cm |
| Cold helium plenum: outer / inner diameter / height | 166 / 50 / 10 cm |
| Holes from cold plenum to pebble bed | 460 × Ø25 (mm; unit not printed, Uncertain) |
| Side reflector graphite, equivalent thickness | 77.8 cm |
| Side boronated carbon brick, equivalent thickness | 22.2 cm |
| Control rod channels: number / diameter / radial position | 10 / 13 cm / r = 102 cm |
| Small absorber ball channels: number / size / radial position | 7 / 16 × 6 cm / r = 98.6 cm |
| Irradiation channels: number / diameter / radial position | 3 / 9.2 cm / r = 98.6 cm |
| Cold helium flow channels: number / diameter / radial position | 20 / 8 cm / r = 144.6 cm |
| Cone-shaped bottom reflector height | 37.5 cm |
| Bottom reflector thickness | 121.2 cm |
| Bottom boronated / non-boronated carbon brick thickness | 30 / 70 cm |
| Reflector graphite density | 1.76 g/cm³ |
| Boronated carbon brick density, including B₄C | 1.59 g/cm³ |
| Hot helium plenum: equivalent outer / inner diameter / height | 166 / 112 / 25 cm |
| Holes from core to hot plenum | 640 × Ø16 (mm; unit not printed, Uncertain) |
| RPV: shell inner diameter / thickness / total height / weight / material | 420 cm / 8 cm / 1115.5 cm / 167 t / SA516-70 |
| Core vessel (barrel): inner diameter / thickness / height / material | 382 cm / 3 cm / 737 cm / SA387-11 |
| Pebble: ball diameter / fuel-zone diameter / matrix graphite density | 6.0 cm / 5.0 cm / 1.73 g/cm³ ([T1] Tab 2.2) |
| Pebble: U loading / enrichment | 5.0 g / 17 % ([T1] Tab 2.2) |
| Kernel radius / UO₂ density | 0.25 mm / 10.4 g/cm³ ([T1] Tab 2.2) |
| Coatings from the kernel outward: material / thickness / density | PyC/PyC/SiC/PyC; 0.09/0.04/0.035/0.04 mm; 1.1/1.9/3.18/1.9 g/cm³ ([T1] Tab 2.2) |
| Dummy ball diameter / density | 6.0 cm / 1.73 g/cm³ ([T1] Tab 2.2) |
| Fuel : dummy loading ratio | 57 : 43 ([T1] Tab 2.2) |
| Fuel-centre temperature definition | Fuel sphere split into 5 zones; the average of the central zone (outer diameter 0.3 cm) is the "centre temperature" ([T1] Sec 2.1.1.6) |

**Derived checks on Tab 2.3.**

- Graphite and carbon-brick radii: 90 + 77.8 = **167.8 cm**, and
  167.8 + 22.2 = **190.0 cm**. These match the model mesh lines at 167.5 and
  190 cm (Section 3.2).
- RPV: 420/2 = **210 cm**, and 210 + 8 = **218 cm**. These match the mesh lines
  at 210 and 218 cm.
- Core vessel: 382/2 = **191 cm**, and 191 + 3 = **194 cm**. The model places
  the vessel between the mesh lines at 194 and 197 cm. That implies an inner
  diameter of 388 cm. Conflict N5 in Section 12; the reading of the figure is
  **Uncertain**.

### 3.2 Heat-conduction model: 44 regions ([T1] Sec 2.2.1.2, Tab 2.4, Fig 2.5)

[T1] Sec 2.2.1.2 describes the INET THERMIX model: a 2-D axisymmetric R-Z
conduction model with **33 radial × 57 axial mesh points** and **44
calculating regions**. The pebble bed, reflectors and gas cavities are
homogenised media, with heat capacity weighted by void fraction. Conduction,
radiation and natural convection are all included. (Quoted)

**Mesh lines**, read from the axis labels printed on Fig 2.5 (Quoted, from
legible printed labels):

```text
r (cm), 33 lines: 0 6.5 13 19 25 39 50 60 70.5 83.5 90 95.6 108.6 140.6 148.6 167.5 178.8
                  190 194 197 210 218 283.4 283.5 285.4 287.4 288.6 300 320 420 520 540 560
z (cm), 57 lines: -1939 -1739 -1539 -1339 -1139 -939 -839 -789 -739 -411 -391 -351 -343 -219
                  -170 -130 -75 -65 -40 0 18 36 54 72 90 108 126 144 162 180 187.5 195 202.5
                  210 217.5 222.2 227 232 263.5 295 325 340 360 400 440 446 461 484 507 657
                  665 772 792 867 942 962 982
```

The counts (33 and 57) match the text. The pair 283.4 / 283.5 is printed as
shown; whether 0.1 cm is intended is **Uncertain**.

**Boundary conditions** ([T1] Sec 2.1.1.4, Quoted): the water cooling panels
(region 39) are held at **50 °C**, and so is the concrete (region 15). Regions
25 and 40 are the hot sink and the cold sink ([T1] Sec 2.1.1.3).

**Region table.** Number, name, material and solid mass fraction are Quoted
from [T1] Tab 2.4. The **extent** column was read from Fig 2.5 and is
**Uncertain** throughout. Fig 2.5 is drawn in mesh-index space: every mesh
interval has the same width on the page, whatever its length in cm. Each
boundary line was assigned to the printed mesh line it sits on. The method is
supported by the Derived checks in Section 3.1 (carbon brick, RPV), by the cone
height below, and by the riser-ring check in Section 3.4. Regions whose
extent could not be resolved say so.

| No. | Zone ([T1] Tab 2.4) | Material | Mass fraction | Extent read from Fig 2.5 (r; z), cm |
|---|---|---|---|---|
| 1 | Pebble bed | Fuel elements | 0.61 | r 0–90, z 0–180; stepped cone to r 25 at z 217.5; discharge tube r 0–25 down to z 484 |
| 2 | Side reflector without flow path | Graphite | 1.0 | r 108.6–140.6 and 148.6–167.5 (side); r 25–39 and 50–140.6 at z 325–340 (bottom) |
| 3 | Side reflector with control rod channels | Graphite | 0.793 | r 95.6–108.6 |
| 4 | Side reflector with coolant flow paths | Graphite | 0.862 | r 140.6–148.6 |
| 5 | Carbon brick insulator | Carbon bricks | 1.0 | r 167.5–190 (side); r 25–39 and 50–140.6 at z 340–440 (bottom) |
| 6 | RPV | Steel | 1.0 | r 210–218 (side); z 657–665 (bottom head) |
| 7 | Core vessel | Steel | 1.0 | r 194–197 (see N5) |
| 8 | Upper bottom reflector with flow paths | Graphite | 0.769 | r 39–60, z about 210–227 |
| 9 | Lower bottom reflector with flow paths | Graphite | 0.932 | r 39–90, z 232–295 |
| 10 | Reactor bottom steel supporting structure 1 | Steel | 0.5 | r 50–190, z 461–507 |
| 11 | Reactor bottom steel supporting structure 2 | Steel | 1.0 | r 50–190, z 440–446; r 25–39, z 440–507 |
| 12 | Reactor bottom annular cold helium flow path | Helium | 0.01 | r 39–50, z 461–507 |
| 13 | Leaking flow zone in steel structures | Steel | 0.96 | r 39–50, z 325–446 |
| 14 | Side reflector without coolant flow path | Graphite | 1.0 | r 90–95.6 |
| 15 | Concrete | Concrete | 1.0 | z 792–942 (bottom); further extent not resolved |
| 16 | Side fluid boundary | — | 1.0 | outer radial band, about r 420–520; not resolved |
| 17 | Thermal insulation | — | 1.0 | z 772–792 (bottom); a top element near r 197–210 is not resolved |
| 18 | Helium cavity inside RPV | Helium | 1.0 | above the top reflector, about z −343 to −219; not resolved |
| 19 | Gas plenum at core entry | Helium | 1.0 | r 0–90, z −40 to 0 |
| 20 | Reactor bottom gas rounding flow paths | Helium | 1.0 | r 39–148.6, z 446–461 |
| 21 | Bottom helium cavity inside RPV | Helium | 1.0 | r 0–210, z 507–657 |
| 22 | Null | — | — | — |
| 23 | Side helium gap between core vessel and RPV | Helium | 1.0 | r 197–210, above z 295 |
| 24 | Air cavity outside RPV | Air | 1.0 | r 218–283.4 |
| 25 | Hot gas plenum at core bottom exit | Helium | 1.0 | r 0–90, z 295–325 |
| 26 | Bottom air cavity outside RPV | Air | 1.0 | z 665–772 |
| 27 | Side cavity of reactor cavity | Air | 1.0 | about r 288.6–300; not resolved |
| 28 | Small plenum in bottom reflector | Helium | 1.0 | r 39–108.6, z 227–232 |
| 29 | Entry throttle of cooling gas for fuel discharging tube | Graphite | 0.993 | r 0–25, z 484–507 |
| 30 | Exit throttle of cooling gas for control rods | Graphite | 0.998 | r 95.6–108.6, z 222.2–227 |
| 31 | Bottom reflector without flow paths | Graphite | 1.0 | r 25–39, z 217.5–295; r 60–90 below the cone steps to z 227 |
| 32–34 | Null | — | — | — |
| 35 | Reactor bottom fluid boundary | — | — | z 942–982 |
| 36 | Top air cavity outside RPV | Air | 1.0 | above the RPV head; not resolved |
| 37 | Top air cavity of reactor cavity | Air | 1.0 | above 36; not resolved |
| 38 | Side helium gap inside core vessel | Helium | 1.0 | r 190–194 |
| 39 | Water cooling panels | — | — | thin band near r 285–288; not resolved |
| 40 | Helium entry flow path inside RPV | Helium | 1.0 | r 197–210, z 295–325 |
| 41 | Annular flow path inside RPV | Helium | 1.0 | r 197–210, z 325–507 |
| 42 | Air gap outside insulating plates of reactor cavity | Air | 1.0 | near r 283.5–285.4; not resolved |
| 43 | Helium plenum in top reflector | Helium | 1.0 | z −75 to −65 |
| 44 | Helium flow zone in top reflector | Graphite | 0.911 | r 0–90, z −65 to −40 |

**Derived check, cone height.** The stepped cone runs from z = 180 to
z = 217.5, in six steps of 7.5 cm (at r = 83.5, 70.5, 60, 50, 39, 25).
217.5 − 180 = **37.5 cm**, which equals the Tab 2.3 cone height.

### 3.3 Convection model: 19 flow regions ([T1] Sec 2.2.1.2, Tab 2.5, Fig 2.6)

The KONVEK gas model has **19 flow regions**. [T1] Sec 2.2.1.2 says it uses
**18 radial and 38 axial mesh points**. Fig 2.6 prints **18** radial labels but
only **36** axial labels (conflict N4).

```text
Fig 2.6 r (cm), 18 lines: 0 6.5 13 19 25 39 50 60 70.5 83.5 90 95.6 108.6 140.6 148.6 197 210 560
Fig 2.6 z (cm), 36 lines: -1939 -435 -65 -40 0 18 36 54 72 90 108 126 144 162 180 187.5 195
                          202.5 210 217.5 222.2 227 232 263.5 295 325 340 360 400 440 446 461
                          484 507 657 982
```

Void fractions are Quoted from [T1] Tab 2.5. Extents were read from Fig 2.6 and
are **Uncertain** (same method as Section 3.2; Fig 2.6 is the cleaner drawing).

| No. | Zone ([T1] Tab 2.5) | Void fraction | Extent read from Fig 2.6 (r; z), cm |
|---|---|---|---|
| 1 | Pebble bed | 0.39 | r 0–90, z 0–180, the cone steps to z 217.5, and the discharge tube r 0–25 down to z 484 |
| 2 | Flow zone in the upper bottom reflector | 0.23 | r 39–60 between the cone steps and z 227 |
| 3 | Flow zone in the lower bottom reflector | 0.068 | r 39–90, z 232–295 |
| 4 | Hot helium plenum at the core bottom exit | 1.0 | r 0–90, z 295–325 |
| 5 | Cold helium plenum at the core entry | 1.0 | r 0–90, z −40 to 0 |
| 6 | Non-flowing zone | — | everything not otherwise labelled, including r 25–39 below the cone and z 657–982 |
| 7 | Cavity of the RPV bottom closure | 1.0 | r 0–210, z 507–657 |
| 8 | Annular coolant flow path at reactor bottom | 0.99 | r 39–50, z 461–507 |
| 9 | Gas rounding flow path at reactor bottom | 1.0 | r 39–148.6, z 446–461 |
| 10 | Coolant flow paths in the side reflector (risers) | 0.138 | r 140.6–148.6, z −65 to 446 |
| 11 | Entry throttle of cooling gas for the fuel discharging tube | 0.0067 | r 0–25, z 484–507 |
| 12 | Exit throttle of the control rod channels | 0.0024 | r 95.6–108.6, z 222.2–227 |
| 13 | Control rod channels in side reflector | 0.207 | r 95.6–108.6, z −65 to 222.2 |
| 14 | Helium plenum in the top reflector | 1.0 | r 0–148.6, z −435 to −65 |
| 15 | Small plenum in the bottom reflector | 1.0 | r 39–108.6, z 227–232 |
| 16 | Helium entry flow cavity inside the RPV | 1.0 | r 197–210, z 295–325 |
| 17 | Annular helium flow path inside the RPV | 0.99 | r 197–210, z 325–507 |
| 18 | Flow path zone in the top reflector | 0.089 | r 0–90, z −65 to −40 |
| 19 | Gap leaking flow zone | 0.046 | r 39–50, z 325–446 |

**Flow path in words** ([T1] Sec 2.1.1.3, paraphrased). Helium enters the RPV
(region 16) and flows down the annulus between the core vessel and the RPV
(17). It turns at the bottom: a small part enters the discharge tube through
its throttle (11). Most of it passes the bottom supports (8, 9) and rises
through the 20 side-reflector risers (10) to the plenum in the top reflector
(14). A small part flows down the control-rod channels (13) and leaves through
side holes (12) into the small bottom plenum (15). The main stream passes down
through the bed (18, 5, 1) and the bottom-reflector borings (2, 3), and mixes
in the hot plenum (4) to a 700 °C average.

### 3.4 Consistency between Tab 2.4 and Tab 2.5 (Derived)

Where a region appears in both models, the solid mass fraction in Tab 2.4
should be 1 minus the void fraction in Tab 2.5. Checked:

| Region (Tab 2.4 / Tab 2.5) | Tab 2.4 solid | 1 − Tab 2.5 void | Agrees? |
|---|---|---|---|
| Pebble bed (1 / 1) | 0.61 | 1 − 0.39 = 0.61 | yes |
| Risers (4 / 10) | 0.862 | 1 − 0.138 = 0.862 | yes |
| Control-rod ring (3 / 13) | 0.793 | 1 − 0.207 = 0.793 | yes |
| Upper bottom reflector (8 / 2) | 0.769 | 1 − 0.23 = 0.77 | yes |
| Lower bottom reflector (9 / 3) | 0.932 | 1 − 0.068 = 0.932 | yes |
| Top reflector flow zone (44 / 18) | 0.911 | 1 − 0.089 = 0.911 | yes |
| Discharge-tube throttle (29 / 11) | 0.993 | 1 − 0.0067 = 0.9933 | yes, at the printed rounding |
| CR exit throttle (30 / 12) | 0.998 | 1 − 0.0024 = 0.9976 | yes, at the printed rounding |
| Bottom annular path (12 / 8) | 0.01 (helium) | void 0.99 | yes |
| Gap leaking zone (13 / 19) | 0.96 | 1 − 0.046 = 0.954 | **no**, off by 0.006 |

**Void fractions rebuilt from the Tab 2.3 hole counts (Derived).**

- **Risers.** 20 holes of Ø8 cm in the ring r = 140.6–148.6:
  $20 \pi 4^2 / (\pi (148.6^2 - 140.6^2)) = 1005 / 7263 = 0.138$. **Matches**
  Tab 2.5.
- **Top reflector.** 460 holes of Ø2.5 cm over r = 0–90:
  $460 \pi 1.25^2 / (\pi 90^2) = 2258 / 25447 = 0.0887$. **Matches** the
  0.089 of region 18.
- **Control-rod ring, region 13 (0.207). Not reproduced.** In r = 95.6–108.6
  (8341 cm²), the 10 CR holes alone give 0.159. CR plus absorber-ball channels
  give 0.240. CR plus irradiation channels give 0.183. All 20 channels give
  0.264. No combination gives 0.207.
- **Lower bottom reflector, region 3 (0.068). Not reproduced.** 640 holes of
  Ø1.6 cm give 1287 cm². That is 0.051 over r = 0–90 and 0.062 over r = 39–90.

How these two fractions were obtained is **Unknown**.

## 4. Material and correlation data ([T1] Tabs 2.8–2.13, Figs 2.7–2.9)

Every formula below was checked against the rendered PDF pages 27–30 of [T1].
They are written with exactly the printed coefficients. T is in °C unless the
table says K.

### 4.1 Helium ([T1] Tab 2.8) — pressure unit fixed (D5)

As printed (Quoted), with **P and T stated as "pressure [Pa] and temperature
[K]"**:

$$\rho = 48.14 \frac{P}{T} \left(1 + 0.4446 \frac{P}{T^{1.2}}\right)^{-1} \quad [\mathrm{kg/m^3}], \quad \sigma = 0.03 \sqrt{P}\ \%$$

$$c_p = 5195\ \mathrm{J/(kg\,K)}, \quad c_v = 3117\ \mathrm{J/(kg\,K)}, \quad \sigma = 0.05 P^{(0.6 - 0.1 T/T_0)}\ \%$$

$$\eta = 3.674 \times 10^{-7}\, T^{0.7} \quad [\mathrm{Pa\,s}], \quad \sigma = 0.0015\, T\ \%$$

$$\lambda = 2.682 \times 10^{-5} \left(1 + 1.123 \times 10^{-3} P\right) T^{0.71 (1 - 2 \times 10^{-4} P)} \quad [\mathrm{W/(cm\,K)}], \quad \sigma = 0.0035\, T\ \%$$

Stated range for all: 1 MPa ≤ P ≤ 10 MPa, 293 K ≤ T ≤ 1773 K. $T_0$ is not
defined in the table (**Unknown**).

**D5 fix: P must be in bar, not Pa.** The evidence is Derived:

1. **The density coefficient is the bar-to-Pa factor over the helium gas
   constant.** With $R_{He} = 8.314462618 / 0.004002602 = 2077.26$ J/(kg K),
   $10^5 / R_{He} = 48.140$. That is the printed 48.14 to all printed digits.
   The leading term $48.14 P/T$ is therefore the ideal-gas density with P in
   bar.
2. **The Pa reading is unphysical.** At 3 MPa and 523.15 K, P = 3e6 "Pa" gives
   ρ = 378 kg/m³. In the conductivity, $1 + 1.123 \times 10^{-3} \times 3 \times 10^6 = 3370$
   and the exponent $0.71(1 - 600)$ is negative, so λ = 0.
3. **The bar reading is physical.** At 30 bar and 250 °C it gives
   ρ = **2.741** kg/m³ against the ideal-gas 2.761 (−0.7 %). It gives
   λ = **0.2299** W/(m K) and η = **2.939e-5** Pa s. At 30 bar and 700 °C it
   gives ρ = **1.479** kg/m³ (ideal 1.484) and λ = **0.356** W/(m K).

**Use P in bar in every Tab 2.8 formula, including the σ expressions.** The
range "1–10 MPa" then reads as 10–100 bar. This is a correction to the source
and must be cited as such wherever it is implemented.

**Related record.** `htr10-plant-data.md` Sec 7.3 recorded the [S5] version of
these fits with the conductivity "internally inconsistent" and the pressure
units "not stated". Tab 2.8 settles both (Section 12.3).

### 4.2 Fuel element and pebble bed ([T1] Tab 2.9)

All Quoted from [T1] Tab 2.9.

- **Density:** 1720 kg/m³.
- **Volumetric heat capacity**, "for all graphite, German company HRB",
  T ≤ 1200 °C:

```text
Cv = 1.75 * (0.645 + 3.14e-3*T - 2.809e-6*T^2 + 0.959e-9*T^3)      [J/(cm3 K)], T in degC
```

- **Fuel-element conductivity**, including fast-neutron irradiation,
  valid 450–1300 °C:

```text
lambda = ((-0.3906e-4*T + 0.06829) / (DOSIS + 1.931e-4*T + 0.105) + 1.228e-4*T + 0.042) * 1.2768
         [W/(cm K)], T in degC
if T <= 450 degC then T = 450;  if DOSIS > 2.5 then DOSIS = 2.5
DOSIS = fast-neutron dose in units of 1e21 (the fluence unit, e.g. n/cm2, is not printed: Unknown)
```

- **Effective conductivity of the pebble bed**, the "German Breitbach
  correlation":

```text
lambda_eff = 1.1538e-6 * T1^1.6622     [W/(cm K)],  T1 = T + 100,  T in degC
if T < 250 degC then T = 250
```

**Derived values**, in W/(m K):

| T (°C) | Breitbach bed | Fuel element, DOSIS 0 | Fuel element, DOSIS 1 |
|---|---|---|---|
| 300 | 2.44 | 46.2 (evaluated at the 450 °C clamp) | 17.9 |
| 500 | 4.79 | 44.1 | 18.4 |
| 800 | 9.39 | 36.1 | 21.7 |
| 1000 | 13.11 | 33.6 | 23.9 |

### 4.3 KTA heat transfer and pressure drop ([T1] Tab 2.10)

All Quoted from [T1] Tab 2.10. The table does not cite KTA by number. The
names "KTA 3102.2" and "KTA 3102.3" are the brief's labels. [T1] Sec 2.2.2.4
and `htr10-plant-data.md` Sec 7.3 call them KTA correlations.

**Pebble-to-gas heat transfer:**

$$Q = \alpha A_K (T_K - T_G), \quad \alpha = \frac{Nu\, \lambda}{d}, \quad Re = \frac{(\dot{m}/A)\, d}{\eta}$$

$$Nu = 1.27 \frac{Pr^{1/3}}{\epsilon^{1.18}} Re^{0.36} + 0.033 \frac{Pr^{1/2}}{\epsilon^{1.07}} Re^{0.86}$$

The symbols:

- A is the flow area of the empty bed.
- $A_K$ is the surface area of one fuel element.
- ε is the bed void fraction, written in the table as $\epsilon = f(D/d)$
  without a formula.
- $\dot{m}$ is the gas flow in the bed.
- $T_K$ is the average fuel-element surface temperature.
- $T_G$ is the helium temperature.
- d is the element diameter and D is the bed diameter.
- λ and η are the helium conductivity and viscosity.

Applicability: $100 \le Re \le 10^5$, $0.36 \le \epsilon \le 0.42$,
$D/d \ge 20$, $H \ge 4d$. The deviation is ±20 % at 95 % "creditability".

**Frictional pressure loss in the bed:**

$$\frac{\Delta P}{\Delta H} = \Psi \frac{1 - \epsilon}{\epsilon^3} \frac{1}{d} \frac{1}{2 \rho} \left(\frac{\dot{m}}{A}\right)^2$$

$$\Psi = \frac{320}{Re/(1 - \epsilon)} + \frac{6}{\left(Re/(1 - \epsilon)\right)^{0.1}}$$

Applicability: $1 < Re/(1 - \epsilon) \le 10^5$, $0.36 \le \epsilon \le 0.42$,
$H > 5d$. ρ is the helium density.

This **closes the gap** recorded in `htr10-plant-data.md` Sec 7.3 and Sec 9,
where the assembly equation for ψ was "too OCR-degraded to transcribe".

### 4.4 Reflector graphite ([T1] Tab 2.11)

All Quoted from [T1] Tab 2.11, read at 300 dpi.

**Side reflector**, irradiation dependent, 0 ≤ T ≤ 2000 °C:

```text
lambda = 1.15 * (1.5648 - 0.3162*log(T + 100)) * F       [W/(cm K)], T in degC
F  = F1/(Y - F2) + F3,   Y = D/10,   D = fast-neutron dose in units of 1e21
F1 = -0.0054705 + 0.00038214*(T/1000) + 0.13487*(T/1000)^2
F2 = -0.013951  + 0.12064*(T/1000)    - 0.32955*(T/1000)^2
F3 = -0.07264   + 0.41459*(T/1000)    + 0.23149*(T/1000)^2
```

**Top and bottom reflectors**, unirradiated, T ≤ 1700 °C:

```text
lambda = 1.15 * (1 - 1.084*T1 + 0.743*T1^2 - 0.213*T1^3)   [W/(cm K)],  T1 = T/1000, T in degC
```

**The `log` must be base 10 (Derived).** The base is not printed. With the
natural log, at T = 500 °C the factor is 1.5648 − 0.3162 ln 600 = **−0.458**,
which gives a negative conductivity. With log10 it is **+0.686**.

**Sign check (Derived).** The text extraction dropped the leading minus signs
on F1, F2 and F3. They are present on the rendered page. With them, F at zero
dose is between 0.98 and 1.16 for T from 300 to 1000 °C, so an unirradiated
side reflector is close to the "1.15 ×" prefactor.

**Derived values**, in W/(m K):

| T (°C) | Side, D = 0 | Side, D = 1 | Top/bottom |
|---|---|---|---|
| 300 | 84.2 | 11.6 | 84.6 |
| 500 | 77.5 | 31.7 | 71.0 |
| 800 | 75.4 | 55.3 | 57.4 |
| 1000 | 80.2 | 67.7 | 51.3 |

The reflector graphite volumetric heat capacity is **not given** in Tab 2.11.
The Tab 2.9 Cv says "for all graphite", which reads as covering it (Uncertain
interpretation).

### 4.5 Carbon brick ([T1] Tab 2.12)

All Quoted from [T1] Tab 2.12.

```text
Cv     = 1.55 * (0.645 + 3.14e-3*T - 2.809e-6*T^2 + 0.959e-9*T^3)  [J/(cm3 K)], T <= 1200 degC
lambda = 0.05 + 0.03e-3*T                                         [W/(cm K)],  T <= 1000 degC
```

That is 5.0 W/(m K) at 0 °C and 8.0 W/(m K) at 1000 °C (Derived).

### 4.6 Steels ([T1] Figs 2.8, 2.9) — figures only

[T1] gives the steel properties **only as curves**. The values below were read
by eye from the rendered figure. Fig 2.8 was read at 300 dpi by pixel position
against the axis ticks. All are **Uncertain**, about ±0.5 W/(m K) and
±0.1e6 J/(m³ K).

| Curve | Conductivity, Fig 2.8 | Volumetric heat capacity, Fig 2.9 |
|---|---|---|
| "SA240, SS316, Core Barrel" (solid) | about 13.3 at 293 K, 18.8 at 671 K, 23.6 at 1072 K, nearly linear | about 3.9e6 at 300 K rising to 5.05e6 J/(m³ K) at 1070 K |
| "SA508, SA516-70, RPV" (dashed) | about 37.5 at 300 K, a maximum near 38.6 around 400–480 K, 35.5 at 700 K | about 3.5e6 at 300 K rising to 4.8e6 J/(m³ K) at 700 K |

**Material-label conflict.** Tab 2.3 names the core vessel **SA387-11**.
Fig 2.8 labels the core-barrel curve **SA240, SS316**. See N16 in Section 12.

### 4.7 Emissivities ([T1] Tab 2.13)

All Quoted from [T1] Tab 2.13:

| Cavity | ε |
|---|---|
| Cold helium plenum in the top reflector | 0.85 |
| Annular flow path inside the RPV | 0.79 |
| Side helium gap inside the core vessel | 0.82 |
| Hot gas plenum at the core bottom exit | 0.85 |
| Side helium gap between the core vessel and RPV | 0.85 |
| Air cavity outside the RPV | 0.85 |
| Gas plenum at the core entry | 0.85 |
| Helium cavity inside the RPV | 0.79 |

**Missing from the benchmark:** property data for the insulation and the
thermal shield ([T1] Sec 7.1.2, Quoted as a recommendation). These are
**Unknown**.

### 4.8 Afterheat ([T1] Fig 2.7) — figure only

[T1] Fig 2.7 is INET's afterheat curve, "normalized power %" against 0–10 h.
[T1] Sec 2.1.1.6 says participants may compute their own. Read by eye from the
rendered page (**Uncertain**, about ±0.15 percentage points):

| t (h) | 0 | 0.5 | 1 | 2 | 4 | 6 | 10 |
|---|---|---|---|---|---|---|---|
| Afterheat (% of normalising power) | about 5.3 | about 2.0 | about 1.6 | about 1.35 | about 1.0 | about 0.9 | about 0.75 |

**Unknown:** the normalising power (10 MW, or the 3 MW test level), the decay
standard used, and the curve before 0.1 h. [T1] Sec 2.3.2.6 notes the standard
behind the decay curve should have been stated. The operating history before
the tests is given only as "approximately 3000 MW-hour" with xenon at
equilibrium ([T1] Sec 2.1.1.6).

### 4.9 What already exists in the workspace

Checked before writing (search-before-building):

- [`htr10.md`](./htr10.md) Sec 3 records KTA implementations in
  `crates/outram-park-digital-twin-engine/src/htr10/kta.rs` and
  `crates/tampines/src/gas_phase/kta_bed.rs`. They were **not compared against
  Tab 2.10 in this pass**. The later tiers should do that before writing a
  third.
- A grep of `crates/**/*.rs` on 2026-11-03 for the Tab 2.8/2.9 coefficients
  (`1.1538`, `1.6622`, `2.682e-5`, `0.4446`) found **no** implementation of the
  Tab 2.8 helium fits or the Breitbach fit.

## 5. Full-power steady state (FPIC) — the benchmark specification (D4)

### 5.1 Operating point

| Quantity | Value | Source | Confidence |
|---|---|---|---|
| Thermal power | 10 MW | [T1] Tab 2.1 | Quoted |
| Primary helium pressure | 3.0 MPa | [T1] Tab 2.1 | Quoted |
| Inlet / outlet helium | 250 / 700 °C | [T1] Tab 2.1 | Quoted |
| Helium mass flow (Rated Coolant Flow Rate) | 4.32 kg/s | [T1] Tab 2.1, Sec 2.1.1.3 | Quoted |
| Energy-balance flow, $10^7 / (5195 \times 450)$ | 4.278 kg/s, 1.0 % below the rated 4.32 | — | Derived |
| Power density map | [T1] Tab 2.6, with the (z = 180, r = 83.5–90) cell revised from 1.03 to **0.0** W/cm³ ([T1] Sec 2.1.1.6) | `htr10-th-data/power-density-fpic-10mw-tab2-6.csv` | Quoted |
| No heat source | for z > 180 cm, in the initial core and in the discharge tube | [T1] Sec 2.1.1.5–2.1.1.6 | Quoted |
| Volume integral of Tab 2.6 | **9.984 MW** | — | Derived |
| Fixed temperatures | water cooling panels and concrete, 50 °C | [T1] Sec 2.1.1.4 | Quoted |

**How the power map is laid out (Derived).** Tab 2.6 prints 11 r values
(0 … 90) but only 10 numbers per row, and 12 z values (0 … 187) but only 11
rows. Sec 2.1.1.6 calls the revised entry "the cell (Z = 180 cm,
R = 83.5~90 cm)". So the printed values are **cell edges**, and each number
applies from its z to the next z and from its r to the next r.

The volume integral over those cells is 9.984 MW, which supports this reading.
The CSV uses this layout.

### 5.2 The 22 thermocouples ([T1] Tabs 2.7, 2.14, 7.1, 7.2)

Positions are from [T1] Tab 2.7. Measured values and the INET result (to
0.1 °C) are from Tab 2.14. All other participants are from Tab 7.1. All Quoted.

The full table, with every participant column, is in
[`htr10-th-data/fpic-thermocouples-tab7-1.csv`](./htr10-th-data/fpic-thermocouples-tab7-1.csv).

| # | Location | r | z | Exp (°C) | INET (°C) | CRP AVG ± STD printed (°C) |
|---|---|---|---|---|---|---|
| 1 | Side reflector | 193 | 80 | 231.3 | 243.9 | 235 ± 17 |
| 2 | Side reflector | 189 | 80 | 249.3 | 263.5 | 262 ± 14 |
| 3 | Side reflector | 167 | 80 | 274.3 | 291.6 | 294 ± 13 |
| 4 | Side reflector | 133 | 80 | 289.1 | 316.4 | 322 ± 12 |
| 5 | Side reflector | 117 | 80 † | 313.3 | 349.5 | 353 ± 15 |
| 6 | Side reflector | 93 | 80 | 357.7 | 410.4 | 430 ± 29 |
| 7 | Side reflector | 193 | 170 | 234.9 | 247.3 | 239 ± 16 |
| 8 | Side reflector | 189 | 170 | 264.8 | 270.9 | 270 ± 13 |
| 9 | Side reflector | 167 | 170 | 303.9 | 305.3 | 308 ± 21 |
| 10 | Side reflector | 133 | 170 | 328.5 | 341.5 | 352 ± 19 |
| 11 | Side reflector | 117 | 170 | 365.3 | 388.8 | 406 ± 25 |
| 12 | Side reflector | 93 | 170 | 507.1 | 501.3 | 550 ± 57 |
| 13 | Top reflector | 60 | −40 | 245.7 | 260.8 | 271 ± 9 |
| 14 | Hot helium plenum | 40 | 234 | 800.2 | 769.9 | 771 ± 33 |
| 15 | Hot helium plenum | 60 | 234 | 763.1 | 714.1 | 725 ± 37 |
| 16 | Bottom carbon brick | 70 | 440 | 224.1 | 239.5 | 251 ± 5 |
| 17 | Bottom carbon brick | 50 | 400 | 245.7 | 263.7 | 274 ± 58 |
| 18 | Bottom carbon brick | 50 | 370 | 296.2 | 295.4 | 275 ± 21 |
| 19 | Bottom carbon brick | 50 | 340 | 406.7 | 339.8 | 338 ± 49 |
| 20 | Fuel discharge tube | 26 | 340 | 334.2 | 316.8 | 322 ± 30 |
| 21 | Fuel discharge tube | 26 | 300 | 806.1 | 707.4 | 595 ± 169 |
| 22 | Fuel discharge tube | 26 | 260 | 881.7 | 792.7 | 762 ± 70 |

† **z of TC 5 is blank in Tab 2.7** (checked on the rendered page). The 80 cm
is from Tabs 2.14 and 7.1.

**Measurement uncertainty: not provided** ([T1] Sec 7.1.1.2, Quoted). For the
same reason [T1] says no acceptance criterion could be set. The participants'
engineering judgement was that agreement within 10 % (50–100 K) "should be
achievable" (Sec 7.1.1.2, paraphrased).

**Derived check on the Tab 7.1 statistics** (recomputed from the printed
columns). For every row, the printed **AVG** reproduces as the mean of **all
11** submitted values, including UK-WIMSTER. The printed **STD** reproduces as
the sample standard deviation of the **10 values excluding UK-WIMSTER**.

The difference matters only where WIMSTER is an outlier:

- TC 2: printed 14, all-11 gives 27.4.
- TC 8: printed 13, all-11 gives 34.0.

So AVG and STD are taken over different sets (finding N2). Quote the
participant columns, not the AVG ± STD, when a spread is needed.

**Operating state of the measurement (finding N9).** [C14] Sec 3 says the
steady-state test was taken during the 72 h full-power run. [C14] Tab 2 gives
that run's operational values (Quoted):

- 10.3 MW
- inlet / outlet 236 / 700 °C
- 2.94 MPa
- feedwater / steam 99 / 430 °C
- feed flow 3.56 kg/s
- steam pressure 3.45 MPa

The benchmark spec is 10 MW at 250 / 700 °C and 3.0 MPa. Under D4 the spec is
the base case. The as-run point is case **SS-ASRUN** (Section 9).

### 5.3 Computed maxima ([T1] Tabs 2.15, 7.3)

These are code results, not measurements. All Quoted.

| Component | INET (Tab 2.15) | CRP AVG ± STD (Tab 7.3) | Range of the submitted values (Tab 7.3) |
|---|---|---|---|
| Fuel centre | 988.2 °C | 941 ± 48 | 881 (SA-VSOP) to 1026 (Turkey) |
| Fuel surface | 924.5 °C | 903 ± 61 | 812 (SA-VSOP) to 1008 (Turkey) |
| Side reflector surface | 511.8 °C | 577 ± 116 | 447 (Korea) to 836 (Turkey) |
| Bottom reflector surface | 789.3 °C | 761 ± 75 | 581 (Turkey) to 833 (UK-WIMSTER) |

Tab 7.3 column blanks, confirmed on the rendered page:

- Japan has no fuel values.
- SA-TINTE has no reflector values.

The printed AVGs reproduce as the means of the 10 values in each row (Derived:
941.3, 902.9, 576.9, 760.6).

**Korea's fuel surface (973 °C) is printed higher than its fuel centre
(929 °C).** That is impossible for a heat-generating sphere, so the Korea
column is **Uncertain** (finding N3).

The pebble-bed temperature profiles (r = 0, 45, 90; z = 0, 80, 170) were
asked for in the benchmark but were **never measured**. They exist only as
calculated curves (Figs 2.10, 2.11, 7.1, 7.2) and are not transcribed.

## 6. Transient tests — initial states (as-run, D4)

Both tests were run at nominally 30 % power. The LOFC test was run on
15 October 2003 ([H06] Sec 1, [C09] abstract). The CRW test was run twice
([H06] Sec 4.2).

| Quantity | **Chosen (as-run)** | Basis | Alternatives (sensitivity cases, Section 9) |
|---|---|---|---|
| Thermal power | **3315 kW** | [J09] Tab 2 (Quoted). [C09] Tab 1 gives 3.3 MW, consistent. [C09] Sec 5's "24.7 % … about 820 kW" implies 820/0.247 = 3320 kW (Derived), also consistent. | 3000 kW: [H06] Sec 2.1, 4.1, 4.2 (**TR-HU**). 3 MW: [T1] Sec 2.3.1.1, 2.4.1 (**TR-SPEC**). |
| Primary pressure | **2476 kPa** | [J09] Tab 2 (Quoted). [C09] Tab 1: 2.5 MPa | 2500 kPa: [H06], [T1] spec |
| Core inlet helium | **212 °C** | [J09] Tab 2 and [C09] Tab 1 (Quoted) | 215 °C: [H06] Sec 4.1, 4.2 (**TR-HU**). 250 °C: [T1] spec (**TR-SPEC**). |
| Core outlet helium | **650 °C** | [J09], [C09], [H06] Sec 4, [T1] (all agree) | — |
| Helium mass flow | **1.457 kg/s** | **Derived** from the chosen state, $3.315 \times 10^6 / (5195 \times (650 - 212))$. No source prints a measured flow. | 1.413 kg/s: [J09] Sec 2.4, which says it "was used to adjust the initial steady state conditions", i.e. a **tuned** value (**TR-FLOW-J09**). 1.444 kg/s from the [T1] spec. 1.328 kg/s from the [H06] state (Derived). |
| Pebbles in the core at the test | 13,651 fuel + 16,408 dummy | [J09] Tab 2 (Quoted) | none stated elsewhere |
| Power distribution | LOFC: [T1] Tab 2.56. CRW: [T1] Tab 2.61. | Quoted. Volume integrals 2.991 MW and 2.999 MW (Derived). | Tab 2.56 and 2.61 differ in one cell (N1). |
| Xenon | at equilibrium at test start | [T1] Sec 2.3.1.1; [J09] Sec 2.3 | — |
| Prior operation | about 3000 MWh | [T1] Sec 2.3.1.1 | the duration and modes are **Unknown** ([T1] Sec 7.1.4) |
| RCCS boundary | 50 °C at the water cooling tubes | [J09] Sec 2.4 (Quoted); [T1] Sec 2.1.1.4 | — |
| Surface cooling system heat removal | 206 kW, a pre-test analysis input | [H06] Sec 2.1, citing the unpublished Liang 2003 | The power level and temperatures at which it holds are **Unknown**. |
| SG water inventory | 40 kg, a pre-test analysis input | [H06] Sec 2.1 | — |

**The chosen flow is derived, not fitted.** [J09]'s 1.413 kg/s was tuned to
reproduce the initial state, so under the process rule it is a sensitivity
case, never the default. The derived 1.457 kg/s is the total flow. How it
splits between core and bypass is Section 10.

**Note on the dummy count (Derived).** 13,651 fuel balls at 57:43 imply
13,651 × 43/57 = 10,298 mixed dummies. The other ~6,110 dummies are then in
the cone and the discharge tube. That is consistent with the loading described
in [T1] Sec 2.1.1.5. The split is not stated anywhere (Unknown).

## 7. Loss of forced cooling without scram (LOFC ATWS)

### 7.1 Event sequence

| Item | As run | Benchmark spec ([T1] Sec 2.3.1.1) |
|---|---|---|
| t = 0 | Helium circulator switched off; it coasts down ([C09] Sec 4; [H06] Tab 1) | Blower stopped, flow stopped and secondary isolated, **all instantaneous at t = 0** |
| t = 12 s | Protection system acts on the signal "ratio of water flow to helium flow is high": the flapper (baffle) valve closes and the secondary loop is isolated. No control rod moves. ([H06] Tab 1; [C09] Sec 4) | — |
| Control rods | none moved ([H06], [C09], [T1]) | none moved |
| Circulator coastdown | shown **only as figures**: [C09] Fig 4 (blower speed) and [J09] Fig 6, the latter selected by matching the short-term power decay | — |

~~**Unknown:** the coastdown curve as numbers. An as-run LOFC case therefore
needs [C09] Fig 4 or [J09] Fig 6 digitised, which was **not done here**. Until
it is, the only fully defined LOFC input is the spec case **LOFC-SPEC**. That
departure from D4 is forced by missing data, and is stated as such.~~
**CORRECTED 2026-11-03 (gh:#1151):** both are now digitised:
[`lofc-blower-speed-c09-fig4.csv`](./htr10-th-data/lofc-blower-speed-c09-fig4.csv)
(the **measured** blower speed, the as-run coastdown under D4) and
[`lofc-flow-coastdown-j09-fig6.csv`](./htr10-th-data/lofc-flow-coastdown-j09-fig6.csv)
([J09]'s tuned GAMMA+ input flow, a sensitivity case only). The as-run LOFC
is therefore fully defined; **LOFC-SPEC** is a sensitivity case.

### 7.2 Measured response (all Quoted; conflicts in Section 12)

| Quantity | Value | Source |
|---|---|---|
| Power to 1 % of initial | within 330 s (THERMIX and test both) | [C09] Sec 5; [C14] Tab 4 |
| Power to decay-heat level | about 700 s | [H06] Tab 1, Sec 4.1 |
| Re-criticality | **2520 s** ([H06] Tab 1) / **4075 s** ([T1] Sec 2.3.2.1) | conflict L1 |
| First power peak | 0.25 at **4325 s** ([T1] Sec 2.3.2.1); 0.25 at about **4300 s** ([T1] Sec 7.1.3); 24.7 %, about 820 kW, at **4400 s** ([C09] Sec 5, [C14] Sec 3.3); 25 % at **4400 s** ([J09] Sec 3.1) | conflict L2 |
| Second peak | 0.1 at 1250 s after the first ([T1] Sec 2.3.2.1); 0.098 at about 1200 s after ([T1] Sec 7.1.3) | conflict L3 |
| Long-term power | about 200 kW after about 7200 s ([H06] Tab 1); about 180 kW within 3 h ([C09] Sec 5) | conflict L4 |
| Top reflector | +162 °C in the first 2 h ([H06] Sec 4.1) | — |
| Top internals TCs (averages of symmetric pairs) | TR1&TR4: 232 → 405 °C in 3 h. TR2&TR5: +52 °C from 218 °C. | [C09] Sec 5 |
| Side reflector SR6 | minimum 295 °C at 2800 s (THERMIX: 335 °C at 4200 s) | [C09] Sec 5 |
| Fuel-centre maximum (calculated) | 883 °C (THERMIX post-test, [C09]); about 823 °C (pre-test, [H06] Sec 2.2) | code results |
| Full traces | figures only: [T1] Figs 2.82–2.84, 7.3, 7.4; [H06] Fig 3; [C09] Figs 4, 6, 8–11; [J09] Fig 7 | ~~not digitised~~ **CORRECTED 2026-11-03 (gh:#1151):** [C09] Figs 4, 6 (measured power, blower speed) and 8–11 (measured thermocouples) digitised into `htr10-th-data/lofc-*.csv`; the others are not |

[T1] Tab 2.59 is the **South African TINTE calculation** of the LOFC power,
not a measurement. It is not transcribed.

**THERMIX sensitivity record ([C14] Tab 4, Quoted).** These are a precedent
for the sensitivity cases in Section 9.

| Case | Time to 1 % | Re-criticality | First peak time | First peak value |
|---|---|---|---|---|
| 1, base | 330 s | 3090 s | 4220 s | 32.6 % |
| 2, temperature coefficient −50 % | 390 s | 3680 s | 5260 s | 38.8 % |
| 3, residual heat −50 % | 330 s | 2230 s | 3275 s | 31.9 % |
| 4, with xenon | 330 s | 3605 s | 4800 s | 29.8 % |

## 8. Control-rod withdrawal without scram (CRW ATWS)

### 8.1 Reactivity insertion ([T1] Tab 2.60)

[T1] Tab 2.60 gives the inserted reactivity against time, 0–128 s in 1 s
steps, in mk. All 129 points are in
[`htr10-th-data/crw-reactivity-tab2-60.csv`](./htr10-th-data/crw-reactivity-tab2-60.csv).
They were checked to be monotone non-decreasing.

Key points (Quoted), with dollar values Derived using β = 0.726 %:

| t (s) | ρ (mk) | ρ ($) |
|---|---|---|
| 1 | 0.0825 | 0.011 |
| 10 | 1.0542 | 0.145 |
| 20 | 1.6788 | 0.231 |
| 53 | 3.3371 | 0.460 |
| 128 | 4.9762 | 0.685 |

The slope is about 0.108 mk/s up to 10 s, which is **$0.0149/s** (Derived).
[H06] Sec 4.2 says the rod moved at 10 mm/s and the maximum reactivity rate
was $0.014/s (Quoted). These agree to within rounding.

Tab 2.60 describes the **large** insertion. [J09] Sec 2.3 gives the same
4.9762 mk over 128 s with the "5 mk control rod". [H06] gives $0.689 in 128 s.
Converted, the Tab 2.60 end point is $0.685 (Derived; the 0.6 % difference is
in the conflict table as C1). [C14] Sec 3 describes it as 5 ‰ Δk/k.

**Small insertion.** [J09] Sec 2.3 says a 1 mk rod was withdrawn in **20 s**.
[H06] says $0.138 (or $0.136; finding N6) in **10 s**. 1 mk is $0.138
(Derived). **No time table for the small insertion is published**; [J09] shows
it only in its Fig 5 (Unknown).

### 8.2 Event sequence

| Item | As run | Benchmark spec ([T1] Sec 2.4.1) |
|---|---|---|
| t = 0 | One control rod withdrawn at 10 mm/s ([H06] Sec 4.2) | withdrawal per Tab 2.60 |
| Trip | At 12 s the protection system acts on "power increasing rate exceeds 3.5 %/s": blower off, flapper closed, secondary isolated; the rod keeps moving ([H06] Tab 2, Sec 4.2). [J09] Sec 2.3: the circulator trips when power reaches 3.96 MW, 120 % of 3.3 MW. | Flow stopped and primary isolated **instantly at t = 9 s**. Inlet, outlet and flow held at the initial values for 0–9 s. |
| Trip signals expected | "power exceeds 4800 kW" and "power increasing rate exceeds 3.5 %/s" ([H06] Sec 4.2) | — |

D4 makes **t_trip = 12 s** the base case. The spec 9 s is case **CRW-TRIP9**.

### 8.3 Measured response (all Quoted)

| Quantity | Small insertion | Large insertion |
|---|---|---|
| First peak, relative | 150 % at 23 s ([H06] Tab 2); 149 % at 22 s ([J09] Sec 3.2) | 216 % at 35 s ([H06] Tab 2); about 2.4 at 31 s ([T1] Sec 7.1.5); 241 % at 30 s ([J09] Sec 3.3) |
| First peak, absolute | 5037 kW ([H06] abstract) | 7230 kW ([H06] abstract) |
| Power to decay level | 700 s | 810 s ([H06] Tab 2) |
| Re-criticality | 2520 s ([H06] Tab 2) | 2400 s ([H06] Tab 2); first re-criticality about 3400 s ([T1] Sec 2.4.2.1) |
| Re-criticality peak | 25 % at 4200 s ([J09] Sec 3.2) | 0.28 at about 3400 s ([T1] Sec 7.1.5); 27 % at 3420 s ([J09] Sec 3.3) |
| Long-term power | about 200 kW at about 7200 s ([H06] Tab 2) | same |
| Top reflector rise | — | +215 °C in the first 2 h ([H06] Sec 4.2); +212 °C measured ([J09] Sec 3.3) |
| Interval between the first and second re-criticality peaks | — | 1200 s ([T1] Sec 7.1.5) |

Traces: figures only. [T1] Figs 2.104, 2.105, 7.5, 7.6; [H06] Figs 5, 6;
[J09] Figs 10–14. Not digitised.

**How the peak conflict likely arises (Derived; finding N7).** Divide [H06]'s
absolute peaks by its stated 3000 kW initial power: 5037/3000 = **168 %** and
7230/3000 = **241 %**. Those disagree with its own relative 150 % and 216 %.
Divide instead by [J09]'s 3315 kW: **152 %** and **218 %**, which match [H06]'s
relative values. [J09]'s 241 % equals 7230/3000.

So the 216 % against 241 % conflict is most likely **one measurement normalised
by two different initial powers**. This is an inference from arithmetic. It is
not verified against the plant record, so it is **Uncertain**.

## 9. Kinetics data and the sensitivity cases

### 9.1 Kinetics

| Quantity | **Chosen** | Source | Alternatives |
|---|---|---|---|
| β (total) | **0.726 %** | [T1] Sec 2.3.1.1, 2.4.1; [J09] Tab 3; [C09] Tab 1 (7.26e-3); [H06] Sec 2.1 (7.26e-3) | — (all agree) |
| Prompt generation time Λ | **0.00168 s** | [T1] Sec 2.3.1.1 ("prompt generation time"); [J09] Tab 3 ("effective prompt lifetime"); [C09] Tab 1 ("prompt neutron lifetime", 1.68e-3 s) | **1.68e-4 s**: [H06] Sec 2.1, "prompt neutron lifetime" (**KIN-LAMBDA-H06**) |
| α fuel / moderator / reflector | **−2.13 / −16.2 / +7.71 pcm/°C** | [T1] Sec 2.3.1.1, 2.4.1; [J09] Tab 3 (×1e-5 Δk/k/°C) | below |
| Six-group delayed-neutron constants | **Unknown** | [T1], [C09] and [J09] say six groups are used, but none prints the group constants | — |
| How α is applied | an overall core-average coefficient, split over core zones by power fraction ([T1] Sec 2.1.1.6). [G02] Sec 4.3.2 uses 5 axial zones. | Quoted | — |

Alternative temperature coefficients (all Quoted):

| Case | Fuel | Moderator | Reflector | Source |
|---|---|---|---|---|
| **KIN-ALPHA-G02-IC** | −1.42 | −10.8 | +5.14 pcm/°C | [G02] Sec 4.3.2, initial core |
| **KIN-ALPHA-G02-EQ** | −2.55 | −5.62 | +4.29 pcm/°C | [G02] Sec 4.3.2, equilibrium core |
| **KIN-ALPHA-C09** | one overall −1.4e-4 Δk/k/°C, i.e. −14 pcm/°C (Derived) | — | — | [C09] Tab 1 |
| **KIN-ALPHA-H06** | $ −1.93e-5 /°C | $ −1.49e-5 /°C | $ +7.08e-6 /°C | [H06] Sec 2.1, printed in dollars |

**[H06]'s coefficients are Uncertain (finding N8).** Read literally, in
dollars per °C × β, the fuel value is −1.4e-7 Δk/k/°C, i.e. −0.014 pcm/°C:
about 150 times smaller than every other source. Read instead as Δk/k/°C, they
are −1.93 / −1.49 / +0.708 pcm/°C. The fuel value is then close to [T1]'s
−2.13, but the moderator and reflector values are about one tenth of [T1]'s.
Neither reading reproduces [T1]. The sum of [T1]'s three is −10.62 pcm/°C,
against [C09]'s overall −14 (Derived).

### 9.2 Named sensitivity cases

The base case uses the "Chosen" entries above. Each named case changes **one**
input, so its effect can be read alone.

| Case | What changes from the base | Why it exists |
|---|---|---|
| **SS-ASRUN** | FPIC at 10.3 MW, inlet 236 °C, 2.94 MPa ([C14] Tab 2); flow by energy balance, 4.273 kg/s (Derived) | The thermocouples were measured during this run (N9) |
| **TR-HU** | Transients start at 3000 kW, inlet 215 °C, 2500 kPa ([H06]) | [H06]'s as-run state |
| **TR-SPEC** | Transients start at 3 MW, 250 / 650 °C, 2.5 MPa ([T1] spec) | The benchmark definition |
| **TR-FLOW-J09** | Initial flow 1.413 kg/s ([J09]) | A tuned value, kept only to measure its effect |
| **LOFC-SPEC** | Flow stop and isolation instantaneous at t = 0 | The spec; ~~also the only fully defined LOFC input until a coastdown is digitised~~ (**CORRECTED 2026-11-03**: the coastdown is digitised, Sec 7.1) |
| **CRW-TRIP9** | Flow stop and isolation at 9 s instead of 12 s | The spec against [H06] |
| **CRW-PMAP-0** | The Tab 2.61 cell (z 180–187, r 83.5–90) set to 0.0, as in Tab 2.56 | Finding N1 |
| **KIN-LAMBDA-H06** | Λ = 1.68e-4 s | The [H06] value |
| **KIN-ALPHA-G02-IC**, **-G02-EQ**, **-C09**, **-H06** | The temperature coefficients in the table above | Source disagreement |
| **DH-INET** / **DH-OWN** | The INET afterheat curve ([T1] Fig 2.7, once digitised) against an independently computed one | [T1] Sec 2.1.1.6 invites both; [C14] Tab 4 shows re-criticality is sensitive to it |
| **XE-OFF** / **XE-ON** | Xenon feedback excluded or included | [C14] Tab 4 case 4; [H06] Sec 5 |
| **BYP-*** | The bypass splits in Section 10 | Unmeasured (D5) |
| **HE-COOLPROP** | Helium properties from the workspace's CoolProp port instead of Tab 2.8 | Lets a pressure-unit error in Tab 2.8 be caught by comparison |

**Rule for using these cases.** None of them is a free parameter. Each takes
its value from a cited source. Report every case's result, including the ones
that move the answer away from the measurement. **A case must not be chosen
afterwards because it fits.**

## 10. Bypass split — unmeasured (D5)

**[T1] Sec 7.1.2 (Quoted):** no coolant bypass flow measurements are
available, so the participants made different assumptions. [T1] Sec 7.1.7
repeats that the bypass flows are not known.

| Path | [T1] Sec 2.1.1.3 (spec, assumed) | [G02] Sec 4.2 (safety analysis) | [S5] (recorded in `htr10-plant-data.md` Sec 7.4, not re-read) |
|---|---|---|---|
| Pebble bed (effective cooling) | **87 %** of 4.32 kg/s | **86 %** | at least 86 % |
| Fuel discharge tube | 1 % | included in the 14 % | 1 % |
| Control-rod channels | 2 % | included in the 14 % | about 2.5 % |
| Leakage through graphite gaps, cold inlet straight to hot outlet, non-cooling | **10 %**, the stated "maximum", assumed conservatively | included in the 14 % | under 10 % |

**Recorded as an assumption, not a fact.** The base case uses the [T1] split
(87/1/2/10) for the steady state, per D4. It is an assumed split, not a
measurement. Named cases:

- **BYP-G02**: 86 % to the bed, the other 14 % spread over the three paths in
  [T1]'s proportions, because [G02] does not split them.
- **BYP-S5-CR**: control-rod flow 2.5 %, from [S5], with the leakage reduced to
  9.5 % to conserve flow.
- **BYP-NOLEAK**: the 10 % leakage set to zero. This is the physical bound on
  the other side, because [T1] calls 10 % a maximum.

**Do not tune the bypass to fit the thermocouples.** Report all four cases.

## 11. Open-only subset (redistributable sources only)

If the proprietary sources ever had to be dropped, this is all that remains.
Everything here is from [K17], [P21] or [S21] and was re-read for this sheet.

| Item | Value | Source | Confidence |
|---|---|---|---|
| Pebble-bed effective conductivity, irradiation included | $\lambda_{eff} = 2.549 \times 10^{-4}\, T^{1.545} + 1.5$ W/(m K) for T ≤ 1300 °C; $2 \times 10^{-3} (T - 135)^{1.29} + 0.003$ for 1300–2500 °C; T in °C; uncertainty "as high as ±10 %" | [K17] eqs (108)–(109), PDF p. 200 | Quoted. The extracted text of (109) reads "1 3000 C"; the range is 1300 °C, as in (108). |
| Comparison with Breitbach ([T1] Tab 2.9) | [K17] gives 3.21 / 5.27 / 9.29 / 12.5 W/(m K) at 300 / 500 / 800 / 1000 °C; Breitbach gives 2.44 / 4.79 / 9.39 / 13.11 | — | Derived. They agree within 5 % at 800–1000 °C but differ by 24 % at 300 °C. |
| Emissivities | steel, oxidised and polished: 0.88 at 425 °C, 0.92 at 800 °C; CrNi steel: 0.85 at 440 °C, 0.90 at 795 °C; graphite: 0.80 at 630 °C; concrete: < 0.80 at 20 °C, > 0.80 at 100 °C | [K17] Table 54, PDF p. 229 | Quoted |
| RPV steel | SA 516-70 specified for HTR-10; long-term temperatures limited to 370 °C | [K17] PDF p. 111 | Quoted |
| Reflector graphite grade | IG110 for HTR-10 | [K17] PDF p. 119 | Quoted |
| Pebble radius | 3 cm | [P21] Sec 2.1, Tab 2 | Quoted |
| DEM pebble density, fuel / dummy | 1.857 / 1.790 g/cm³ | [P21] Tab 2 | Quoted |
| TRISO per pebble | exactly 8,335 in the model | [P21] Sec 2.1 | Quoted (a model choice) |
| Matrix density; U per pebble; enrichment; kernel boron | 1.73 g/cm³; 5 g; 17 wt %; 4 ppm | [P21] Tab 1 | Quoted |
| TRISO layers as printed | order "Kernel/Buffer/PyC/SiC/PyC"; thickness "Ø=0.5/0.04/0.035/0.04/0.09" mm; density 10.4/1.1/1.9/3.18/1.9 g/cm³ | [P21] Tab 1 | Quoted, but see the check below |
| DEM bed | 28,020 pebbles (24,531 mixed in the core zone) at t = 0; active height 208.12 cm | [P21] Sec 2.1, Tab 3 | Quoted (a model result, not a plant value) |
| Operation history, 2003–2007 | 569 days, 2503.4 MWd: 2003, 106 d / 258.9 MWd; 2004, 168 / 708.5; 2005, 149 / 821.4; 2006, 97 / 532; 2007, 49 / 182.6 | [S21] Tab 1 (citing Chen 2014) | Quoted |
| Pebble counts | first criticality: 16,890 mixed at 57:43, cone and discharge tube filled with graphite pebbles; FPIC: 23,900 mixed; full core (December 2004): about 27,000 | [S21], "The HTR-10 operation history" section | Quoted |
| Full power | 10 MW at 700 °C outlet, January 2003 | [S21], "The HTR-10 operation history" section | Quoted |
| Measured validation quantity | PANGU against measured coolant outlet temperature over the steady power periods: difference "generally below 5 °C" | [S21] abstract, Fig 8 | Quoted. ~~Figure only, not digitised.~~ **CORRECTED 2026-11-03:** digitised, `htr10-th-data/she2021-fig8-outlet-temperature.csv` (gh:#1151) |
| Measured inlet mass flow and inlet temperature, whole history | figures only | [S21] Fig 4 | ~~not digitised~~ **CORRECTED 2026-11-03:** digitised, `she2021-fig4a-inlet-flow.csv`, `she2021-fig4b-inlet-temperature.csv` (gh:#1151) |

**Putra's coating layers are transposed (check, Derived).** [P21] Tab 1 lists
its *densities* kernel-outward: 1.1 is the low-density buffer and 3.18 is SiC
in the fourth slot. Read in the same order, its *thicknesses* make the buffer
0.04 mm and the outer PyC 0.09 mm.

The thickness sequence after the kernel diameter, 0.04/0.035/0.04/0.09, is
exactly [T1] Tab 2.2's 0.09/0.04/0.035/0.04 **reversed**. So [P21]'s
thicknesses appear to be listed outside-in while its densities are listed
inside-out. **Do not use the [P21] thicknesses without this correction.** The
benchmark ordering ([T1] Tab 2.2, buffer 0.09 mm) is the cited one.

**The open sources do not give:**

- any in-core thermocouple temperature;
- any LOFC or CRW trace, timing or initial state;
- the reflector graphite, carbon brick or steel property fits;
- the KTA heat-transfer and pressure-drop correlations;
- the helium property fits (CoolProp is the open route);
- any reflector, plenum, channel, barrel or RPV dimension beyond the RPV steel
  grade;
- the bypass split;
- the kinetics parameters.

## 12. Conflicts

**Choice** follows D4 (as-run for transients, spec for the steady state)
unless stated otherwise. Rows marked **NEW** were found in this pass and are
not in the earlier extraction or in `htr10-plant-data.md`.

### 12.1 Conflicts that change model inputs

| ID | Quantity | Values | Choice | Sensitivity case |
|---|---|---|---|---|
| I1 | Transient initial power | 3315 kW [J09]; 3.3 MW [C09]; 3000 kW [H06]; 3 MW [T1] spec | 3315 kW | TR-HU, TR-SPEC |
| I2 | Transient inlet temperature | 212 °C [J09], [C09]; 215 °C [H06]; 250 °C [T1] spec | 212 °C | TR-HU, TR-SPEC |
| I3 | [H06] Sec 2.1 prints the pre-test state as "outlet 250 °C, inlet 650 °C" | evidently swapped: its own Sec 4 gives inlet 215 and outlet 650 | ignore Sec 2.1's labels | — |
| I4 | Transient flow | 1.457 Derived; 1.413 [J09] tuned | 1.457 kg/s | TR-FLOW-J09 |
| I5 | Λ | 1.68e-3 s [T1], [J09], [C09]; 1.68e-4 s [H06] | 1.68e-3 s | KIN-LAMBDA-H06 |
| I6 | Temperature coefficients | [T1]; [G02] IC and EQ; [C09] overall; [H06] (N8) | [T1] | KIN-ALPHA-* |
| I7 | CRW flow-stop time | 12 s as run [H06]; 9 s spec [T1] | 12 s | CRW-TRIP9 |
| I8 | LOFC isolation | coastdown from 0 with isolation at 12 s, as run; instantaneous at 0, spec | as run (~~once a coastdown is digitised; until then LOFC-SPEC~~ **CORRECTED 2026-11-03**: digitised, Sec 7.1) | LOFC-SPEC |
| I9 | Bypass split | 87/1/2/10 [T1]; 86/14 [G02]; CR 2.5 % [S5] | [T1], as an assumption | BYP-* |
| I10 | FPIC operating point | spec 10 MW / 250 °C / 3.0 MPa; as measured 10.3 MW / 236 °C / 2.94 MPa ([C14] Tab 2) | spec | SS-ASRUN |
| I11 **NEW (N1)** | Power-map cell z 180–187, r 83.5–90 at 3 MW | 0.0 in Tab 2.56 (LOFC); **0.335** in Tab 2.61 (CRW). The two tables are otherwise identical. Tab 2.6's same cell was revised to 0 in Sec 2.1.1.6. | each test uses its own table, as printed | CRW-PMAP-0 (the difference is 8.3 kW, Derived) |
| I12 | Helium P units in Tab 2.8 | printed Pa; must be bar (Section 4.1) | bar | HE-COOLPROP |
| I13 | Buffer thickness | 0.09 mm [T1] Tab 2.2; [P21] transposed (Section 11) | [T1] | — |

### 12.2 Conflicts in the measured targets (they change what a model is judged against)

| ID | Quantity | Values | How to quote it |
|---|---|---|---|
| L1 | LOFC re-criticality | 2520 s [H06]; 4075 s [T1] Sec 2.3.2.1 | Quote both. [H06]'s "critical again" and [T1]'s "recriticality occurs" may mark different events; that is unresolved. |
| L2 | LOFC first peak | 0.25 at 4325 s [T1] Ch 2; about 4300 s [T1] Ch 7; 24.7 % at 4400 s [C09], [C14]; 25 % at 4400 s [J09] | value 0.25 (all agree); time between 4300 and 4400 s |
| L3 | LOFC second peak | 0.1 at +1250 s [T1] Ch 2; 0.098 at +1200 s [T1] Ch 7 | quote both |
| L4 | LOFC long-term power | about 200 kW at about 7200 s [H06]; about 180 kW within 3 h [C09] | quote both |
| L5 **NEW (N7)** | CRW large peak | 216 % [H06] Tab 2; about 2.4 [T1]; 241 % [J09]; 7230 kW [H06] abstract | Most likely one measurement normalised two ways (Section 8.3, Uncertain). Use the absolute **7230 kW**, with the initial power stated alongside. |
| L6 **NEW (N7)** | CRW small peak | 150 % [H06]; 149 % [J09]; 5037 kW [H06] abstract, i.e. 168 % of 3000 kW | as L5: 5037 kW |
| L7 | CRW peak time | 35 s [H06]; 31 s [T1]; 30 s [J09] | quote all three |
| L8 **NEW (N6)** | Small insertion | $0.136 [H06] abstract and Sec 1; $0.138 [H06] Sec 2.3, 4.2, Tab 2; 1 mk = $0.138 [J09] (Derived) | $0.138 |
| L9 | Small insertion duration | 10 s [H06]; 20 s [J09] | Unknown which is right |
| C1 | Large insertion total | $0.689 [H06]; 4.9762 mk = $0.685 [T1], [J09] (Derived) | Tab 2.60 |
| L10 **NEW (N10)** | CRW top reflector rise | +215 °C [H06]; +212 °C [J09] | quote both |
| L11 | CRW re-criticality (large) | 2400 s [H06]; peak 0.28 at about 3400 s [T1]; 27 % at 3420 s [J09] | onset and peak are different events; quote both |

### 12.3 Conflicts and corrections in the benchmark document itself

| ID | Finding | Status |
|---|---|---|
| N2 **NEW** | Tab 7.1: AVG is over all 11 submissions, STD over 10 (UK-WIMSTER excluded) | Derived, reproduced on all 22 rows |
| N3 **NEW** | Tab 7.3: Korea's fuel surface 973 °C > fuel centre 929 °C | Korea column Uncertain |
| N4 **NEW** | Convection mesh: text says 18 × 38 points; Fig 2.6 prints 18 × 36. [S5] (plant-data Sec 8.2) gives 18 × 36. | Fig 2.6 and [S5] agree; the text's 38 is Uncertain |
| N5 **NEW** | Core vessel: Tab 2.3 gives ID 382 cm (r 191–194); Fig 2.5 appears to place it at r 194–197 (ID 388) with a helium gap at r 190–194 | figure reading Uncertain; Tab 2.3 cited |
| N11 **NEW** | Model against spec: the cavity above the bed is 40 cm in the model (z −40 to 0) against 41.7 cm in Tab 2.3; the hot plenum is 30 cm in the model (z 295–325) against the 25 cm "equivalent height" in Tab 2.3 | Both recorded. The model is INET's discretisation, not plant geometry. |
| N12 **NEW** | Helium fit validity: 1–10 MPa ([T1] Tab 2.8) against 0.1–10 MPa ([S5] as recorded in plant-data Sec 7.3) | quote both |
| N13 | Solid mesh: [T1] gives 33 × 57 **points**; [S5] gives 32 × 56 (plant-data Sec 8.2) | consistent if [S5] counts cells (Derived) |
| N14 **NEW** | Tab 2.4 region 13 (0.96) against Tab 2.5 region 19 (1 − 0.046 = 0.954) | a 0.006 mismatch; quote both |
| N15 **NEW** | The CR-ring void 0.207 and the lower-bottom-reflector void 0.068 cannot be rebuilt from the Tab 2.3 hole counts (Section 3.4) | Unknown how they were derived |
| N16 **NEW** | Core-vessel steel: SA387-11 (Tab 2.3) against "SA240, SS316, Core Barrel" (Fig 2.8, 2.9 legends) | quote both. The steel λ for the barrel is the Fig 2.8 curve whatever its label. |
| N17 | Tab 2.7 prints no z for TC 5 | 80 cm from Tabs 2.14 and 7.1 |

**Gaps in `htr10-plant-data.md` that this sheet now closes.** These are new
data from a source that sheet never used, not contradictions of it. Its
Sec 9 lists as unknown, from its five sources:

- core barrel dimensions → Tab 2.3 here (382 / 3 / 737 cm, SA387-11);
- the helium conductivity form and the pressure units → Section 4.1;
- the pressure-drop assembly equation → Section 4.3;
- the reflector graphite and carbon-brick fits → Sections 4.4 and 4.5.

That sheet's Sec 9 has been annotated with pointers here.

**Differences from the scratchpad extraction report.** That report was used
only to find pages. Where it disagrees with the source, this sheet records
what the source shows:

- It gave Tab 2.11's F1 as a plain quadratic. The source has a **negative**
  constant, −0.0054705.
- It gave the CRW large peak conflict as unresolved. Section 8.3 shows a
  likely normalisation cause.
- It did not record N1–N17 above.

## 13. Machine-readable data

Each file has the citation in `#` header lines.

| File | Content | Source |
|---|---|---|
| [`htr10-th-data/fpic-thermocouples-tab7-1.csv`](./htr10-th-data/fpic-thermocouples-tab7-1.csv) | 22 FPIC thermocouples: r, z, measured, INET to 0.1 °C, all 11 participant columns, printed AVG and STD | [T1] Tabs 2.7, 2.14, 7.1 |
| [`htr10-th-data/crw-reactivity-tab2-60.csv`](./htr10-th-data/crw-reactivity-tab2-60.csv) | CRW inserted reactivity, 0–128 s | [T1] Tab 2.60 |
| [`htr10-th-data/power-density-fpic-10mw-tab2-6.csv`](./htr10-th-data/power-density-fpic-10mw-tab2-6.csv) | FPIC power density by cell (cell-edge layout, Section 5.1) | [T1] Tab 2.6 and Sec 2.1.1.6 |
| [`htr10-th-data/power-density-3mw-lofc-tab2-56.csv`](./htr10-th-data/power-density-3mw-lofc-tab2-56.csv) | 3 MW power density, LOFC | [T1] Tab 2.56 |
| [`htr10-th-data/power-density-3mw-crw-tab2-61.csv`](./htr10-th-data/power-density-3mw-crw-tab2-61.csv) | 3 MW power density, CRW | [T1] Tab 2.61 |
| [`htr10-th-data/lofc-blower-speed-c09-fig4.csv`](./htr10-th-data/lofc-blower-speed-c09-fig4.csv) | LOFC measured relative blower speed, 0–500 s (digitised, gh:#1151) | [C09] Fig 4 |
| [`htr10-th-data/lofc-power-short-c09-fig4.csv`](./htr10-th-data/lofc-power-short-c09-fig4.csv) | LOFC measured relative fission power, 0–500 s (digitised) | [C09] Fig 4 |
| [`htr10-th-data/lofc-power-long-c09-fig6.csv`](./htr10-th-data/lofc-power-long-c09-fig6.csv) | LOFC measured relative fission power, 0–10800 s (digitised) | [C09] Fig 6 |
| [`htr10-th-data/lofc-flow-coastdown-j09-fig6.csv`](./htr10-th-data/lofc-flow-coastdown-j09-fig6.csv) | GAMMA+ input flow coastdown, **tuned** (digitised) | [J09] Fig 6 |
| [`htr10-th-data/lofc-thermocouples-c09-figs8-11.csv`](./htr10-th-data/lofc-thermocouples-c09-figs8-11.csv) | 17 measured internals thermocouple traces, 0–10800 s (digitised) | [C09] Figs 8–11 |
| [`htr10-th-data/she2021-fig8-outlet-temperature.csv`](./htr10-th-data/she2021-fig8-outlet-temperature.csv) | measured core outlet temperature, 7 steady periods, with the PANGU power (digitised) | [S21] Fig 8 (CC BY) |
| [`htr10-th-data/she2021-fig4a-inlet-flow.csv`](./htr10-th-data/she2021-fig4a-inlet-flow.csv), [`she2021-fig4b-inlet-temperature.csv`](./htr10-th-data/she2021-fig4b-inlet-temperature.csv) | inlet mass flow and temperature, whole history (digitised) | [S21] Fig 4 (CC BY) |

**Digitised files (gh:#1151).** Each header states the figure, the method
(native-resolution `pdfimages` extraction, frame and tick calibration,
colour selection per pixel column, legend masked, no interpolation) and the
read error. Checks against the papers' text are in
~~`crates/tampines/verification_and_validation/htr10_loop_lofc/README.md`~~ `crates/nee_soon/verification_and_validation/htr10_loop_lofc/README.md` (moved 2026-11-03) Sec 4.

**How they were produced.** A short Python script parsed the
`pdftotext -layout` output of [T1]. It asserted 22 thermocouple rows, an exact
match between the Tab 2.14 and Tab 7.1 measured columns, all 129 Tab 2.60
points, and 11 × 10 cells per power map. It also printed the Derived checks
quoted above: the AVG/STD reproduction, the power integrals and monotonicity.
The script was kept in the session scratchpad, not committed. Its checks are
the ones listed in this sheet.

## 14. Still unknown

- Measurement uncertainty of every thermocouple and of the power signal.
- The bypass split (unmeasured).
- ~~The circulator coastdown and~~ the small-insertion reactivity history as
  numbers (figures only). **CORRECTED 2026-11-03:** the coastdown is
  digitised (Sec 7.1).
- ~~The LOFC and~~ The CRW power traces as numbers (figures only).
  **CORRECTED 2026-11-03:** the LOFC power trace is digitised from [C09]
  Figs 4 and 6 (gh:#1151).
- The six-group delayed-neutron constants.
- The afterheat curve as numbers, its normalisation and its standard.
- The operating history before the tests, beyond "about 3000 MWh".
- Insulation and thermal-shield properties.
- The unit of the DOSIS / D fluence.
- $T_0$ in the Tab 2.8 heat-capacity σ.
- How the 0.207 and 0.068 void fractions were obtained.
- The conditions behind [H06]'s 206 kW.
- The extents of the outer cavity regions in Fig 2.5 (16, 17-top, 18, 27,
  36, 37, 39, 42).
