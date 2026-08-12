# HTR-10 — plant and piping reference data

A single reference sheet of HTR-10 plant, piping and thermal-hydraulic figures
for the offline digital-twin simulator, each figure tagged with its source and a
confidence note.

Companion to [`htr10.md`](./htr10.md) (capability audit and scoping) and
[`htr10-neutronics.md`](./htr10-neutronics.md). This document exists because
IAEA-TECDOC-1382 is a **neutronics** benchmark and carries almost no plant
detail, so the simulator was inventing its piping and secondary-loop geometry.

> **Intended use.** Education, research, capability building, and V&V only. This
> is an offline demonstration with no connection to any operational system. See
> `RESPONSIBLE_USE.md`.
>
> **Status.** Compiled 2026-08-12 from the five sources registered below. Every
> figure here is a *literature reading*, not a validated simulator input. Nothing
> in this document has been checked against a running model.

## 1. How to read this sheet

Each figure carries a source tag and one of three confidence levels:

| Level | Meaning |
|---|---|
| **Quoted** | The number appears verbatim in the cited source. |
| **Derived** | Arithmetic performed here on quoted numbers. The arithmetic is shown so it can be checked. |
| **Uncertain** | Read from a degraded scan, or internally inconsistent, or contradicted by another source. Do not use without re-reading the source PDF. |

**Nothing in this sheet is filled in from general knowledge.** Where all five
sources are silent, the entry says so explicitly in
[Section 9, Still unknown](#9-still-unknown-after-all-five-sources). An unknown
left as unknown is worth more than a plausible invention, because the simulator
would otherwise present an invented number as sourced.

Three of the five sources use degraded scans. Where OCR damaged a figure it is
marked **Uncertain** and the raw reading is reproduced so a human can check it.

## 2. Source register and redistribution status

Redistribution status was determined per document by inspecting its own front
matter, and it is **not** uniform across the set.

### [S1] Wu, Lin and Zhong 2002 — design features (Elsevier, restricted)

Wu Z., Lin D., Zhong D., "The design features of the HTR-10",
*Nuclear Engineering and Design* **218** (2002) 25-32.
URL: `https://www.gen-4.org/gif/upload/docs/application/pdf/2022-12/the-design-features-of-the-htr-10_2002_nuclear-engineering-and-design.pdf`

**Copyright: Elsevier. Not redistributable.** The PDF and its extracted text
live in the gitignored `collaboration/` directory only. Facts extracted from it
are reproduced here under the principle that facts are not copyrightable; the
citation and URL above are what this repository keeps in place of the text.

### [S2] Qin Zhenya 1996 — general design (JAERI-Conf, open)

Qin Zhenya, "General Design of the 10MW HTR", section 3.4 in
**JAERI-Conf 96-010**, *Proceedings of the 3rd JAERI Symposium on HTGR
Technologies*, Japan Atomic Energy Research Institute, 15-16 February 1996,
pp. 149-160. Author affiliation: Institute of Nuclear Energy Technology (INET),
Tsinghua University, Beijing.
URL: `https://www.gen-4.org/gif/upload/docs/application/pdf/2022-12/jaeri_96_010_htr10generaldesign.pdf`

**Citation correction:** the task named this "JAERI-96-010". The document is
**JAERI-Conf 96-010**, a *conference proceedings* volume, not a JAERI report in
the plain `JAERI-` series. Cite it as above.

**Redistribution: committed to the open archive.** Issued by a Japanese
government research agency and published in GIF's public document library; the
retrieved pages carry no copyright line and no distribution restriction.
**Caveat:** the retrieved PDF is an excerpt beginning at proceedings page 149, so
it contains **no volume front matter**, and the volume-level distribution
statement could not be inspected. Committed on the basis of the absence of any
restriction on the retrieved pages plus the government-agency origin.

Committed to:

- `crates/kovan-literature/open/reports/jaeri-conf-96-010-htr10-general-design.pdf`
- `crates/kovan-literature/open/reports/jaeri-conf-96-010-htr10-general-design.json`
- `crates/kovan-literature/generated/markdown/open/jaeri-conf-96-010-htr10-general-design.md`

### [S3] Sunny and Ilas 2010 — SCALE 6 analysis (ANS proceedings, restricted)

Sunny E. E. (University of Michigan), Ilas G. (Oak Ridge National Laboratory),
"SCALE 6 Analysis of HTR-10 Pebble-Bed Reactor for Initial Critical
Configuration", *PHYSOR 2010 — Advances in Reactor Physics to Power the Nuclear
Renaissance*, Pittsburgh, Pennsylvania, 9-14 May 2010, on CD-ROM, American
Nuclear Society, LaGrange Park, IL (2010).
Landing page: `https://www.ornl.gov/file/scale-6-analysis-htr-10-pebble-bed-reactor-initial-critical-configuration/display`

**Redistribution: NOT committed — this one did not match the task's assumption.**
The task anticipated a US DOE laboratory report, freely redistributable. The
document behind that ORNL landing page is in fact an **American Nuclear Society
conference proceedings paper** — its own header reads "on CD-ROM, American
Nuclear Society, LaGrange Park, IL (2010)". It carries no DOE government-rights
notice and no distribution statement of its own; ANS holds proceedings copyright
by default. It is an author copy hosted by ORNL, which does not by itself grant
redistribution rights.

Treated conservatively, exactly like [S1]: PDF and extracted text stay in the
gitignored `collaboration/` directory, and only facts, citation and URL are
committed.

### [S4] McDowell et al. 2011 — HTGR codes and standards (PNNL, open)

McDowell B. K., Nickolaus J. R., Mitchell M. R., Swearingen G. L., Pugh R.,
"High Temperature Gas Reactors: Assessment of Applicable Codes and Standards",
**PNNL-20869**, Pacific Northwest National Laboratory, Richland, Washington,
October 2011. Prepared for the U.S. Nuclear Regulatory Commission under an
Interagency Agreement with the U.S. Department of Energy, Contract
DE-AC05-76RL01830.
URL: `https://www.pnnl.gov/main/publications/external/technical_reports/pnnl-20869.pdf`

**Redistribution: committed to the open archive.** Front matter (cover, title
page, summary) was inspected and carries **no** limited-distribution, Official
Use Only, export-control or proprietary marking; a full-text scan for
"distribution", "disclaimer" and "Official Use Only" returned nothing. Publicly
posted in PNNL's own public technical-reports directory.

Committed to:

- `crates/kovan-literature/open/reports/pnnl-20869-htgr-codes-and-standards.pdf`
- `crates/kovan-literature/open/reports/pnnl-20869-htgr-codes-and-standards.json`
- `crates/kovan-literature/generated/markdown/open/pnnl-20869-htgr-codes-and-standards.md`

### [S5] Gao and Shi 2002 — thermal-hydraulic calculation (Elsevier, restricted)

Gao Zuying, Shi Lei, "Thermal hydraulic calculation of the HTR-10 for the initial
and equilibrium core", *Nuclear Engineering and Design* **218** (2002) 51-64.
PII: S0029-5493(02)00198-X. Institute of Nuclear Energy Technology, Tsinghua
University, Beijing.
URL: `https://www.gen-4.org/gif/upload/docs/application/pdf/2022-11/thermal-hydraulic-calculation-of-the-htr-10-for-the_2002_nuclear-engineering.pdf`

**Copyright: Elsevier. Not redistributable.** Same treatment as [S1] — PDF and
extracted text in gitignored `collaboration/` only, facts and citation here.

This is the most directly useful source for the simulator's physics and gets its
own section below ([Section 7](#7-thermal-hydraulic-modelling-s5)).

### Sources deliberately not re-derived

IAEA-TECDOC-1382 is already in the committed archive
(`crates/kovan-literature/open/reports/iaea-tecdoc-1382-part1.pdf` and
`-part2.pdf`) and is covered by `htr10-neutronics.md`. It is a neutronics
benchmark and is not a source for the plant geometry in this sheet.

## 3. Baseline already established (from [S1])

Recorded here so it is not re-derived. All from [S1]; see that paper for context.

- Steam generator is **once-through**, **30 helical tube bundle modules**, each
  bundle **112 mm diameter**; heat-transfer tubes are **2.25Cr1Mo**, maximum
  design temperature **500 degC**; tube diameter changes between heat-transfer
  sections, with throttles.
- Main helium circulator is a **vertical single-stage centrifugal** machine,
  impeller at the **lower** end of the shaft, drive motor on the **upper**
  section of the same shaft, grease-lubricated bearings, variable speed.
- Hot gas duct is **coaxial**: cold helium returns in the **outer** pipe, hot
  helium flows in the **inner** pipe. Cold helium from the circulator runs along
  the SG vessel inner wall, into the outer coaxial pipe, then into the RPV where
  part of it cools the vessel wall.
- All three pressure vessels (reactor, SG, hot gas duct) are bathed in **cold,
  roughly 250 degC** helium, which is why the pressure boundary stays cool.
- The side-by-side layout is a deliberate safety choice: easier SG and circulator
  maintenance, and **reduced probability of core water ingress after an SG tube
  rupture**.

## 4. Primary circuit geometry

### 4.1 Hot gas duct — the main gap now closed

| Quantity | Value | Source | Confidence |
|---|---|---|---|
| Inner (hot helium) tube diameter | **300 mm** | [S2] section 4 | Quoted |
| Outer (cold helium) tube diameter | **900 mm** | [S2] section 4 | Quoted |
| Annulus radial gap | **300 mm** | derived: $(900 - 300)/2$ | Derived |
| Thermal insulation | Installed **between** the inner and outer tubes | [S2] section 4 | Quoted |
| Inner-tube pressure duty | High temperature, **low pressure differential** — the cold annulus carries the pressure | [S2] section 4 | Quoted |
| Flow direction | Hot core-to-SG in the centre tube; cold circulator-to-core in the annulus | [S2] sections 4-5, [S5] section 2 | Quoted (both agree) |
| Orientation | **Horizontal** coaxial duct joining the two vessels | [S5] section 2 | Quoted |
| Void volume in the reflector region | $0.70686 \times 10^{5}$ cm3 | [S3] Table VI | Quoted |
| Duct **length** | *Not stated in any of the five sources* | — | Unknown |

[S2] gives the annulus flow area as derived $\frac{\pi}{4}(0.900^2 - 0.300^2) =
0.5655$ m2, and the inner tube flow area as $\frac{\pi}{4}(0.300)^2 = 0.0707$ m2
(**Derived**; both ignore the insulation thickness, which is stated to exist but
never dimensioned, so both are upper bounds).

[S4] independently confirms the arrangement without numbers: "a hot gas duct
inside the cooler gas duct", described as a **cross-vessel** of the same type
first used on Peach Bottom Unit 1 and proposed for GT-MHR. [S4] also records that
INET calls this component a *vessel* but performed its leak-before-break analysis
treating it as a *pipe*.

### 4.2 Pressure vessels

| Quantity | Value | Source | Confidence |
|---|---|---|---|
| Reactor pressure vessel height | **more than 11 m** | [S2] section 2 | Quoted (a bound, not a figure) |
| Reactor pressure vessel diameter | **more than 4 m** | [S2] section 2 | Quoted (a bound, not a figure) |
| Reactor total weight | **more than 300 t** | [S2] section 2 | Quoted (a bound) |
| SG pressure vessel height | **more than 11 m** | [S2] section 3 | Uncertain — scan reads "1 lm", read as 11 m by analogy with the RPV line |
| SG pressure vessel diameter | **2.6 m** | [S2] section 3 | Quoted |
| RPV design pressure | **3.5 MPa** | [S5] section 1 | Quoted |
| RPV accident pressure limit | **3.85 MPa** | [S5] section 1 | Quoted |
| Safety valve setpoints | **3.5 MPa** and **3.75 MPa**, two valves on the primary circuit | [S5] section 1 | Quoted |
| RPV material | **C-Mn-Si steel** | [S4] section 2.7, citing Yu 2011 | Quoted (secondary citation) |
| Annulus between RPV and core barrel | Filled with **250 degC** cold helium to hold vessel temperature below limit | [S5] section 2 | Quoted |

### 4.3 Core and internals

| Quantity | Value | Source | Confidence |
|---|---|---|---|
| Core diameter | **1.8 m** | [S2] Table 1, [S5] section 2, [S3] section 1 | Quoted (all agree) |
| Average core height | **1.97 m** | [S2] Table 1, [S5] section 2, [S3] section 1 | Quoted (all agree) |
| Core volume | **5 m3** | [S2] Table 1 | Quoted |
| Mean power density | **2 MW/m3** | [S5] section 2, [S4] section 2.7 | Quoted (both agree) |
| Fuel pebble diameter | **60 mm** | [S2], [S5], [S3] | Quoted (all agree) |
| Heavy metal per pebble | **5 g**, 17 wt% U-235 | [S2] Table 1, [S5] section 2, [S3] Table II | Quoted (all agree) |
| Pebble count, equilibrium | **27 000** | [S2] section 2, [S5] section 2 | Quoted (both agree) |
| Pebble count, initial core | **13 500** fuel elements, graphite balls 50% of the bed | [S5] sections 2 and 4.2 | Quoted |
| Pebble packing fraction | **0.61** | [S3] Table I | Quoted |
| Multi-pass recirculation | **5 passes** | [S2] Table 1 | Quoted |
| Fuel discharge tube diameter | **500 mm** ([S2]) / **50 cm** ([S5]) | [S2] section 2, [S5] section 2 | Quoted (agree) |
| Fuel discharge tube length | **about 3.3 m** | [S2] section 2 | Quoted |
| Discharge tube RPV penetration diameter | **65 mm** (tube narrows through a discharge facility) | [S2] section 2 | Quoted |
| Fuel loading tube diameter | **62 mm** ([S2]) / **65 mm** ([S5]) | [S2] section 2, [S5] section 2 | **Conflict — see Section 8** |
| Coolant boreholes in side reflector | **20** | [S2] section 5, [S5] section 2, [S3] section 2 | Quoted (all three agree) |
| Control rod channels | **10**, in the side reflector | [S2] Table 1, [S5] section 2 | Quoted (agree) |
| Absorber ball (KLAK) channels | **7** | [S2] Table 1, [S5] section 2, [S3] section 2 | Quoted (all agree) |
| Irradiation / measuring channels | **3** | [S2] Table 1 | Quoted; [S3] counts "13 control rod/irradiation channels", consistent with $10 + 3$ |
| Reflector block layers along core height | **20** ([S2]) / **15** ([S5]) | [S2] section 2, [S5] section 2 | **Conflict — see Section 8** |
| Reflector segments per layer | **20** | [S2] section 2 | Quoted |
| Reflector surround | Graphite reflector, then carbon bricks containing B4C acting as both insulation and absorber | [S2] section 2 | Quoted |
| Boronated carbon brick B4C content | **5 wt%**, brick density 1.59 g/cm3 | [S3] Table II | Quoted |
| Bottom reflector | Contains the hot gas plenum; flow passage split into two sections with a larger flow-area ratio at the centre, to cut core-outlet temperature spread | [S5] section 2 | Quoted |

Channel void volumes from the [S3] KENO VI model, useful as a cross-check on any
reflector geometry the simulator builds:

| Region | Volume ($10^{5}$ cm3) | Source | Confidence |
|---|---|---|---|
| Coolant channels (20) | 5.07681 | [S3] Table VI | Quoted |
| Control rod / irradiation channels (13) | 7.76484 | [S3] Table VI | Quoted |
| KLAK channels (7) | 2.29410 | [S3] Table VI | Quoted |
| Hot gas duct | 0.70686 | [S3] Table VI | Quoted |

### 4.4 Primary coolant path, in order

From [S2] section 5 and [S5] section 2, which agree on the sequence:

1. Helium circulator raises cold helium pressure.
2. Cold helium flows through the annular space between the **SG pressure vessel
   and its sleeve**, outside the thermal insulation.
3. Into the **annulus of the coaxial hot gas duct**.
4. Into the RPV, through the space between the **reactor pressure vessel and the
   core barrel**, down to the bottom of the reactor support structure, cooling
   the support structure first.
5. Splits: a fraction to the control rod holes, a fraction to the fuel discharge
   tube, the rest up the **20 cold-helium boreholes** in the side reflector.
6. Into the **top cold helium plenum**, then **downward** through the pebble bed.
7. Out via the hot helium boreholes into the **hot helium plenum** in the bottom
   reflector, mixing to an average **700 degC**.
8. Through the **centre tube** of the hot gas duct to the steam generator.
9. Cooled 700 degC to 250 degC in the SG, then through **6 connection tubes** to
   the circulator inlet, closing the loop.

The **6 connection tubes** from SG outlet to circulator inlet are from [S2]
section 5 and are corroborated by [S2] Fig. 2, labelled "Connection Tubes(6)".
Their diameter and length are **not stated**.

## 5. Steam generator

| Quantity | Value | Source | Confidence |
|---|---|---|---|
| Type | Once-through, modular helical tube | [S1], [S2] section 3 | Quoted (agree) |
| Number of modules | **30** | [S1], [S2] section 3 | Quoted (agree) |
| Bundle diameter per module | **112 mm** | [S1] | Quoted |
| Tube length | **34 m** per tube | [S2] section 3 | Quoted, but see the ambiguity note below |
| Total heat transfer area | **56 m2** | [S2] section 3 | **Uncertain — arithmetically implausible, see below** |
| Tube material | **2.25Cr1Mo** | [S1] | Quoted |
| Tube maximum design temperature | **500 degC** | [S1] | Quoted |
| Module placement | Installed in an **annular space** in the SG vessel | [S2] section 3 | Quoted |
| Vessel centre | Reserved for a **N2-He intermediate heat exchanger (IHX)** for a later gas-turbine and process-heat phase; **empty in the first stage** | [S2] section 3 | Quoted |
| Phase-2 IHX temperatures | Helium inlet/outlet would rise from 250/700 degC to **300/900 degC** | [S2] section 3 | Quoted |
| Status as of 2011 | SG still installed; plans existed to fit an IHX in the existing cavity with a Brayton turbine, or replace the SG with a direct Brayton turbine, working fluid nitrogen or helium | [S4] section 2.7 | Quoted |
| SG nodalisation used in [S5] | **17 water nodes and 17 helium nodes**, split into sub-cooled / evaporation / superheat sections | [S5] section 3.2.4 | Quoted |
| Tube diameter and wall thickness per section | *Not stated in any of the five sources* | — | Unknown |
| Coil pitch | *Not stated in any of the five sources* | — | Unknown |
| Number of tubes per module | *Not stated* — see ambiguity note | — | Unknown |

**Ambiguity in the [S2] tube-length sentence.** The sentence reads: "There are 30
small helical tubes modules which are installed in the annular space, each tube
is 34m long, the total heat transfer area is 56m2". It does not say whether a
module contains one tube or many, so "each tube is 34 m" cannot be turned into a
total tube length.

**The 56 m2 figure does not survive an arithmetic check and should not be used
as-is.** Two independent checks:

- Average heat flux would be $10\ \text{MW} / 56\ \text{m}^2 = 179$ kW/m2
  (**Derived**). That is very high for a gas-heated surface.
- If the 30 modules held one 34 m tube each, total tube length is 1020 m
  (**Derived**), and $56 = \pi d \times 1020$ gives a tube outside diameter of
  17.5 mm (**Derived**) — a plausible tube size, but only under the
  one-tube-per-module reading, which the source does not support.

The raw PDF text was re-checked directly and prints "56m2", so this is not an
extraction error introduced here — it is what the scan shows. The scan of [S2] is
poor elsewhere (it renders 440 degC as "4 4 0 1" and 250 degC as "250t"), so a
dropped digit in the original scan is possible but **not established**. Treat the
SG area as **unresolved**; re-read the source page before using it, and do not
size the simulator's SG from it.

## 6. Secondary loop and plant conditions

| Quantity | Value | Source | Confidence |
|---|---|---|---|
| Reactor thermal power | **10 MW** | [S2] Table 1, [S5] section 1, [S4] section 2.7 | Quoted (all agree) |
| Primary helium pressure | **3.0 MPa** | [S2] Table 1, [S5] section 1, [S4] section 2.7 | Quoted (all three agree) |
| Helium mass flow | **4.3 kg/s** ([S2], [S5] section 1) / **4.32 kg/s** ([S5] Tables 1-2) | [S2] Table 1, [S5] | Quoted; 4.3 is the rounded statement of 4.32 |
| Core helium inlet temperature | **250 degC** | [S2] Table 1, [S5], [S4] | Quoted (all agree) |
| Core helium outlet temperature | **700 degC** | [S2] Table 1, [S5], [S4] | Quoted (all agree) |
| Circulator pressure rise | **about 0.6 bar** (60 kPa) | [S2] section 5 | Quoted — **conflicts with the [S5] computed loop loss, see Section 8** |
| Feedwater temperature | **104 degC** | [S2] Table 1, [S5] section 1 | Quoted (agree) |
| Feedwater pressure | **6.1 MPa** | [S2] Table 1 | Quoted |
| Steam outlet temperature | **440 degC** | [S2] Table 1, [S5] section 1 | Quoted (agree) |
| Steam outlet pressure | **4.0 MPa** | [S2] Table 1, [S5] section 1 | Quoted (agree) |
| Feedwater / steam mass flow | **3.49 kg/s** | [S2] Table 1 | Quoted; equals 12.56 t/h (**Derived**), consistent with the 12.5 t/h already held |
| Design lifetime | **20 years** | [S2] Table 1 | Quoted |
| Average load factor | **50%** | [S2] Table 1 | Quoted |
| Residual heat removal mode | **Natural circulation** | [S2] Table 1 | Quoted |
| Residual heat removal power | **250 kW** | [S2] Table 1 | Quoted |
| Turbine and condenser data | *Nothing beyond the steam and feedwater conditions above appears in any of the five sources* | — | Unknown |

Confinement and building data from [S2] section 6, recorded for completeness
(not simulator inputs):

- Confinement design pressure **1.3 bar**; blowout flap or blast membrane opens
  at **1.1 bar**.
- Primary cavity held at **-150 Pa** relative to the rest of the building.
- Reactor cavity wall thickness **2.3 m**, mainly shielding.
- Ventilation stack height **40 m**.
- Reactor building: **6 floors, 3 underground**, total height **43 m**,
  construction area **6000 m2**, housing 23 nuclear-island systems.

## 7. Thermal-hydraulic modelling [S5]

This section answers a different question from the geometry above: it records
what a *published* thermal-hydraulic calculation of this plant actually used.

**Read the distinction in Section 8.2 before using anything here as geometry.**
Figures in this section are **modelling choices** by Gao and Shi, not design
data, wherever they differ from Sections 4-6.

### 7.1 Code and method

Calculations used **HTRSIMU**, built on the pebble-bed code **THERMIX**
(KFA-Julich, Wolf et al. 1990). [S5] states that all THERMIX models and equations
were **kept unchanged**; HTRSIMU adds a graphical interface and a control-design
platform only. THERMIX is 2-D cylindrical, finite-difference, successive
point-wise over-relaxation, with a steady/transient conduction code coupled to a
quasi-steady convection code.

### 7.2 Nodalisation — directly relevant to the simulator's discretisation

The simulator's pebble bed is currently a single lumped control volume. This is
what a published calculation of the same plant used:

| Model | Discretisation | Source |
|---|---|---|
| 2-D r-z solid heat conduction | **32 radial x 56 axial** mesh points, **44 material regions** | [S5] section 3.2.1 |
| Gas convection | **18 radial x 36 axial** mesh points, **19 flow regions** | [S5] section 3.2.1 |
| Primary circuit | **10 calculating regions, 51 joint nodes** | [S5] section 3.2.3 |
| Steam generator | **17 water nodes + 17 helium nodes**, by sub-cooled / evaporation / superheat section | [S5] section 3.2.4 |
| Fuel pebble, 1-D radial | **5 regions**; region inner diameters **5.0, 3.0, 1.0, 0.3 cm** plus a centre region. Outermost region is the graphite cladding; inner four are the fuelled zone with coated particles | [S5] section 3.2.2 |

What was lumped or homogenised, from [S5] section 3.2.1:

- Pebble bed, reflectors and gas cavities are treated as **homogeneous media**,
  heat capacities reduced by void fraction.
- Coolant flow between fuel elements is treated as **flow in a homogeneous
  medium**.
- Reflector gas channels are modelled as **pipe-flow regions**; plenums are
  modelled as regions with **zero flow resistance**.
- Conduction, radiation and natural convection are all carried in the solid
  model.
- The **core loading cone was not modelled** — [S5] states it should be
  considered but was not, in this calculation.

The [S5] primary-circuit model includes thermal inertia of the **steam generator
vessel, blower and hot gas duct**.

### 7.3 Correlations used

All are from the German **KTA** safety guides. Notably, the pebble-bed pressure
drop uses **KTA 3102.3, not Ergun** — relevant because the workspace's Ergun
variant is currently a `todo!()`.

**Pebble surface heat transfer — KTA 3102.2:**

$$Nu = 1.27 \frac{Pr^{1/3}}{\epsilon^{1.18}} Re^{0.36} + 0.033 \frac{Pr^{1/2}}{\epsilon^{1.07}} Re^{0.86}$$

Validity, quoted: $100 \le Re \le 10^{5}$, $0.36 \le \epsilon \le 0.42$,
$D/d \ge 20$. Reynolds number is formed on pebble diameter with mass flux
$\dot{m}/A$.

*OCR note:* the symbol in the denominators was lost in extraction. It is read
here as the **bed void fraction** on two grounds internal to the document — the
quoted validity range $0.36$ to $0.42$ is a pebble-bed void fraction range, and
the friction correlation below uses the same symbol as $(1 - \epsilon)$. The
exponents 1.18 and 1.07 and the structure are legible. **Confidence: Quoted for
the exponents, Derived for the symbol identification.**

**Pebble bed friction — KTA 3102.3.** The friction factor is legible:

$$\psi = \frac{320}{Re / (1 - \epsilon)} + \frac{6}{\left( Re / (1 - \epsilon) \right)^{0.1}}$$

Validity, quoted: $100 \le Re/(1-\epsilon) \le 10^{5}$,
$0.36 \le \epsilon \le 0.42$, bed height $H \ge 5d$.

*OCR note:* the **pressure-drop assembly equation** that consumes $\psi$ was too
degraded to transcribe (it extracts as "P H = 1 - 3 1 d 1 2m A 2"). It is **not**
reproduced here rather than guessed. Read it off the source PDF page in
`collaboration/` before implementing. **Confidence: Quoted for the friction
factor, Unknown for the assembly equation.**

**Helium properties — KTA 3102.1**, valid $0.1\ \text{MPa} \le P \le 10\
\text{MPa}$ and $293\ \text{K} \le T \le 1773\ \text{K}$:

```text
rho = 48.14 * (P/T) / (1 + 0.4446 * P / T^1.2)      [kg/m3]
Cp  = 5195                                          [J/(kg.K)]
Cv  = 3117                                          [J/(kg.K)]
mu  = 3.674e-7 * T^0.7                              [Pa.s]
```

Pressure units for the density fit are **not stated** in the source text as
extracted — check before use. The **thermal conductivity** fit extracted as
`2.682e-3 * (1 + 1.123e-3) * T^0.71 * (1 - 2.0e-4 P)`, which is
internally inconsistent (the first bracket has a coefficient with nothing to
multiply). **Uncertain — re-read the source page.**

**Effective pebble-bed conductivity — Breitbach formula**, combining conduction
and radiation between balls:

```text
lambda = 1.1538e-6 * (T + 100)^1.6622    [W/(cm.K)],  T >= 250 degC
```

[S5] section 3.3.5. **Quoted.** [S5] section 3.2.1 adds that effective core
structure conductivities, including ball-to-ball radiation, come from
experimentally determined empirical correlations.

**Fuel pebble conductivity** [S5] section 3.3.4, valid 450-1300 degC, German
fuel correlation used because Chinese fuel data was not available at the time;
depends on fast neutron dose `DOSIS` in units of $10^{21}$:

```text
lambda = ((-0.3906e-4*T + 0.06829) / (DOSIS + 1.931e-4*T + 0.105)
          + 1.228e-4*T + 0.042) * 1.2768     [W/(cm.K)]
```

**Uncertain** — the extracted expression has an unmatched grouping and the
document's own OCR is degraded; verify against the source page.

[S5] section 3.3.6-3.3.7 also gives dose-dependent side-reflector graphite
conductivity, plain reactor-graphite conductivity, reflector volumetric heat
capacity and carbon-brick properties. These are recorded in the source; they are
not transcribed here because the extraction quality is not good enough to be
trusted for implementation.

### 7.4 Flow split and bypass

Design guidelines, [S5] section 1:

- At least **1%** of rated flow through the **fuel discharge tube**.
- About **2.5%** of rated flow through the **control rod tubes**.
- Maximum bypass in the gaps between graphite components **less than 10%** of
  rated flow.
- Conservatively, at least **86%** of rated flow passes through the core.

Computed results, [S5] section 4.4: 2.5% through the control rods, 1% through the
fuel discharge tube, at least 86% through the pebble bed. Thermal power of the
fuel elements sitting inside the discharge tube is about **0.1%** of total
reactor power. Bypass is held under 10% because the graphite segments are joined
by **keys and dowels**.

Velocities, [S5] section 4.4:

- Average helium velocity at **core inlet: 1.5 m/s**.
- Maximum helium velocity at **core outlet: 9.2 m/s**.

### 7.5 Pressure drops around the primary loop

[S5] Table 1 — the most directly usable result in the paper for a loop model:

| Component | Pressure drop (kPa) | Coolant flow (kg/s) |
|---|---|---|
| Pebble bed and bottom reflector | 1.3 | 3.77 |
| Coolant pass in side reflector | 0.7 | 3.846 |
| Flow mixture plenums | 6.1 | 4.32 |
| Steam generator | 15.0 | 4.32 |
| Hot gas duct | 4.1 | 4.32 |
| **Total** | **27.2** | **4.32** |

**Confidence: Quoted, with a reconstructed layout.** The table extracted with its
columns interleaved. The parse is confirmed by two internal checks performed
here: the five component drops sum to exactly 27.2 kPa as printed
(**Derived**), and the pebble-bed flow of 3.77 kg/s is 87.3% of 4.32 kg/s
(**Derived**), consistent with the "at least 86% through the core" statement.

Two points worth carrying into the simulator:

- The **steam generator dominates** the loop at 15.0 kPa of 27.2 kPa total.
- The **flow mixture plenums are second** at 6.1 kPa, and [S5] section 4.4 notes
  this was found experimentally and "is not neglected" — a plenum loss a lumped
  model would normally drop entirely.

### 7.6 Computed temperatures and power

Power distribution, [S5] section 4.2:

| Quantity | Initial core | Equilibrium core |
|---|---|---|
| Maximum power density | 2.84 W/cm3 (at $R = 0$, $Z = 90$ cm) | 2.57 W/cm3 |
| Average power per fuel element | 0.74 kW | 0.37 kW |
| Maximum power per fuel element | 1.05 kW | 0.60 kW |

All **Quoted**, [S5] section 4.2.

Equilibrium-core parameters against load, [S5] Table 2:

| Parameter | 100% | 110% | 120% |
|---|---|---|---|
| Average thermal power density (MW/m3) | 2 | 2.2 | 2.4 |
| Coolant pressure (MPa) | 3 | 3 | 3 |
| Helium temperature at reactor inlet (degC) | 250 | 250 | 250 |
| Helium temperature at reactor outlet (degC) | 700 | 745 | 790 |
| Coolant mass flow (kg/s) | 4.32 | 4.32 | 4.32 |
| Maximum fuel temperature (degC) | 918.7 | 982.3 | 1046.6 |
| Maximum fuel surface temperature (degC) | 876.7 | 939.3 | 1001.5 |
| Maximum coolant temperature (degC) | 818 | 876 | 932 |
| Maximum side reflector temperature (degC) | 666.9 | 706.6 | 746 |
| Maximum bottom reflector temperature (degC) | 789.7 | 845 | 900 |
| Pressure drop of reactor core (kPa) | 1.3 | 1.4 | 1.43 |

**Confidence: Uncertain on column assignment.** The values are quoted, but [S5]
Table 2 extracted with its column headers scrambled. Columns were assigned here
by monotonicity — every row increases with load, and the 100% column matches
figures quoted elsewhere in the paper (2 MW/m3 power density, 250/700 degC,
918.7 degC maximum fuel temperature matching the 919 degC in the conclusion,
1.3 kPa core drop matching Table 1). The assignment is consistent on all eleven
rows, but it is a reconstruction, not a direct reading.

Other temperature results, [S5] section 4.3:

- Initial core average temperatures: **fuel 605.7 degC**, **fuel element surface
  581.9 degC**, **graphite cladding 587.9 degC**. **Quoted.**
- Initial core high-temperature zone (above 800 degC): axially **144 to 260 cm**.
  Equilibrium core: **180 to 260 cm**, moving downward. **Quoted.**
- **Internal inconsistency in [S5]:** section 4.3 states the maximum fuel centre
  temperature of the initial core is "about 995 degC", while the section 5
  conclusion states 1049 degC for the initial core and 919 degC for equilibrium.
  Both are recorded; the 919 degC equilibrium value is corroborated by Table 2's
  918.7 degC. The 995 vs 1049 difference for the initial core is **unresolved** —
  plausibly with and without the uncertainty factors of section 4.1, but the
  paper does not say so and that reading is **not** asserted here.

Fuel temperature limits, [S5] section 1: coating integrity experimentally proven
to **1250 degC**; design maximum fuel temperature set at **1230 degC** for normal
and accident conditions. All computed values above are below it.

Uncertainty factors applied in [S5] section 4.1, useful if the simulator ever
reports a peak fuel temperature:

- Burnup peaking factor **1.2** (equilibrium core).
- Manufacturing hot-spot factor **1.05** (dimension, UO2 density, enrichment).
- Heat-transfer calculation uncertainty factor **1.2** on the fuel-surface-to-
  fluid temperature difference.
- Fuel element heat factor **2.0** initial core, **1.0** equilibrium (graphite
  balls generate no heat).
- Power non-uniformity error of 1% raises fuel surface temperature by 0.5 degC
  and centre temperature by 1.3 degC.

## 8. Recorded conflicts between sources

Conflicts are recorded, not resolved. Where the later or more authoritative
source is clear, that is stated; where it is not, that is stated too.

### 8.1 Genuine disagreements

**Reflector block layers along the core height — 20 vs 15.**
[S2] section 2 (1996): "The whole graphite reflector consists of 20 layers of
graphite bricks and carbon bricks. Each layer is divided into 20 segments".
[S5] section 2 (2002): "The ceramic core structure is composed of side, bottom
and top reflectors, which consist of 15 layers of graphite and carbon blocks
along the core height."
**Not resolved.** [S5] is later and describes the built plant, which is weak
evidence for 15, but the two may also be counting different things ([S2] counts
"bricks", [S5] counts "blocks", and [S2]'s count may include layers outside the
core height). Do not pick one silently.

**Fuel loading tube diameter — 62 mm vs 65 mm.**
[S2] section 2: "At the top of reactor core there is one loading tube, the
internal diameter is 62mm."
[S5] section 2: "The fuel element loading tube with a diameter of 6.5 cm is
located at the top of the reactor core".
**Not resolved.** Note [S2] specifies an *internal* diameter while [S5] says only
"diameter", so the two may be inner and outer diameters of the same tube rather
than a true disagreement. Note also that [S2] separately gives **65 mm** as the
diameter of the *discharge* tube where it penetrates the RPV, so one of the two
readings may be a transposition between the loading and discharge tubes.

**Control rod cooling flow fraction — 2% vs 2.5%.**
[S2] section 5: "about 2% of cold coolant will be flowed into the control rods
holes".
[S5] sections 1 and 4.4: "the coolant flow passing through the control rod tubes
is about 2.5% rated flow".
**[S5] is preferred where a single number is needed** — it is the later source,
it is a calculation rather than a design description, and it states 2.5%
consistently in three places (design guideline, uncertainty section, results).
Both are recorded.

**Circulator pressure rise vs computed loop pressure drop — 60 kPa vs 27.2 kPa.**
[S2] section 5: "After through the helium circulator, the pressure of the cold
helium would be increased about 0.6 bar".
[S5] Table 1: total computed primary loop resistance **27.2 kPa**.
**These are not the same kind of number and should not be reconciled by picking
one.** [S2]'s 0.6 bar is a stated circulator design head; [S5]'s 27.2 kPa is a
computed sum of component losses at rated flow. A design head roughly 2.2 times
the computed nominal loss is a margin, not necessarily a contradiction. For a
simulator: use **27.2 kPa** for the loop resistance at rated flow, and **60 kPa**
as the circulator's design capability, not as the operating point.

### 8.2 Design figures vs modelling choices — do not conflate

[S5] is a thermal-hydraulic *calculation*, so some of its numbers are the authors'
modelling simplifications rather than plant data. Where [S5] and the design
sources differ, the following are **modelling choices** and must not be recorded
as plant geometry:

- The **44 material regions**, **32 x 56** and **18 x 36** meshes, **19 flow
  regions**, **10 primary-circuit regions / 51 joints**, **17 + 17** SG nodes and
  the **5-region fuel pebble** are all discretisation choices, not plant
  features.
- The **5-region fuel pebble inner diameters** (5.0, 3.0, 1.0, 0.3 cm) are mesh
  boundaries inside a 6 cm pebble, **not** manufactured layer boundaries. The
  real pebble has a 5 cm fuelled zone in a 6 cm sphere ([S3] Table I: fuel zone
  radius 2.5 cm, pebble radius 3.0 cm) — only the outermost of the five mesh
  regions corresponds to a physical boundary.
- The **86% core flow fraction** is explicitly a *conservative* assumption
  ([S5] sections 1 and 4.1.6), chosen to bound flow-allocation uncertainty. It
  is not a measured or design split. The design guideline figures (2.5% control
  rods, 1% discharge tube, under 10% bypass) sum to under 14%, so the true core
  fraction is expected to be **higher** than 86%.
- The **core loading cone is absent** from the [S5] model by the authors' own
  statement. Any geometry taken from [S5] therefore describes a flat-topped core.

### 8.3 A correction to an existing workspace document

`docs/reactor-scoping/htr10.md` section 2 states the primary loop is "Helium,
approx. 7 MPa". **All three sources that state a primary pressure give 3.0 MPa**
— [S2] Table 1, [S5] section 1, and [S4] section 2.7. The RPV *design* pressure
is 3.5 MPa and the accident limit 3.85 MPa ([S5] section 1), so 7 MPa is not any
pressure in this plant.

This sheet does not edit `htr10.md`; flagging it for the maintainer.

## 9. Still unknown after all five sources

Explicitly unknown. None of these should be invented, and the simulator should
not present a value for them as sourced.

**Steam generator internals**

- Tube outside diameter and wall thickness, in any section. [S1] states the
  diameter *changes* between heat-transfer sections and that throttles are
  fitted, but gives no dimensions.
- Coil pitch, helix diameter, number of turns.
- Number of tubes per module.
- Which sections the tube diameter changes between, and the throttle sizing.
- Total heat transfer area — the only figure available (56 m2, [S2]) fails an
  arithmetic check; see Section 5.
- SG vessel internal arrangement beyond "30 modules in an annular space with a
  central cavity reserved for an IHX" and the 2.6 m vessel diameter.

**Primary loop piping**

- Hot gas duct **length**. Diameters are now known (300 mm / 900 mm); the length
  is not stated anywhere.
- Thickness and material of the hot gas duct thermal insulation. Its existence is
  stated, its dimensions are not.
- Diameter, length and routing of the **6 connection tubes** between the SG
  outlet and the circulator inlet.
- Wall thicknesses of the RPV, SG vessel and hot gas duct vessel.
- Exact RPV and SG vessel heights and diameters — [S2] gives only bounds ("more
  than 11 m", "more than 4 m").
- Core barrel dimensions and the RPV-to-core-barrel annulus gap.
- Elevation differences between components, needed for any natural-circulation
  or buoyancy modelling.

**Circulator**

- Impeller diameter, design speed, speed range, power rating.
- Head-flow characteristic curve. Only a single design point is available: about
  0.6 bar rise ([S2]), against a computed 27.2 kPa loop loss ([S5]).
- Efficiency.

**Secondary side beyond the steam generator**

- Turbine type, stage count, rating, efficiency.
- Condenser pressure, heat duty, cooling arrangement.
- Feedwater heater arrangement, deaerator conditions.
- Feedwater pump rating and characteristic.
- The startup and shutdown loop referenced by [S2] section 6 is named but never
  dimensioned.

Nothing beyond the four boundary conditions already held — 440 degC / 4.0 MPa /
3.49 kg/s steam and 104 degC / 6.1 MPa feedwater — was found in any of the five
documents.

**Thermal-hydraulic detail**

- The KTA 3102.3 **pressure-drop assembly equation** that consumes the friction
  factor. The friction factor itself is recovered; the equation it feeds was too
  OCR-degraded to transcribe and was deliberately not guessed.
- The KTA 3102.1 **helium thermal conductivity** fit — the extracted form is
  internally inconsistent.
- Pressure units for the KTA 3102.1 density fit.
- Reflector graphite and carbon brick property fits — present in [S5] section
  3.3.6-3.3.7 but not transcribed here because extraction quality was too poor.
- Cavity cooling system geometry. It appears as a modelled region in [S5]
  section 3.2.1 but is never dimensioned.

All six of the OCR-limited items above are recoverable by a human reading the
source PDF pages directly. The two Elsevier papers are in `collaboration/`
(gitignored); [S2] is in the committed open archive.

## 10. What this closes and what it does not

Closed by this pass:

- **Hot gas duct inner and outer diameters** — 300 mm and 900 mm ([S2]). This was
  the single largest gap.
- **Primary loop pressure drop budget** — a five-component breakdown totalling
  27.2 kPa ([S5] Table 1), which the simulator can use directly instead of an
  invented loop resistance.
- **The pebble-bed correlations actually used for this plant** — KTA 3102.2 for
  surface heat transfer, KTA 3102.3 for friction (**not Ergun**), Breitbach for
  effective bed conductivity ([S5] section 3.3).
- **A published nodalisation to aim at** — 32 x 56 solid, 18 x 36 gas, 17 + 17 SG
  nodes ([S5] section 3.2), against the simulator's current single lumped volume.
- **Flow split and bypass fractions** — 86% core / 2.5% control rods / 1%
  discharge tube / under 10% gap bypass ([S5]).
- **Computed peak temperatures with load** ([S5] Table 2), usable as a sanity
  target for the core model.
- **Primary pressure corrected to 3.0 MPa**, contradicting `htr10.md`.

Not closed:

- The **entire steam generator tube geometry** — diameters, wall thicknesses,
  coil pitch, tube count. This remains the largest open gap, and the one figure
  available for it does not survive arithmetic. A dedicated INET steam-generator
  paper would be the source to look for next.
- The **whole turbine and condenser side**. Five documents, no data.
- **Hot gas duct length** and all primary pipe lengths and elevations.
