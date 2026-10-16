# Open V&V data and BWR system-level T-H models, for epic #895

Fleet slice: open verification and validation (V&V) data, plus BWR system-level thermal-hydraulic models.
Compiled 2026-10-16 from web search. Fetching NEA, NRC and HAL pages directly failed
(DNS errors from the sandbox), so every access term below comes from search-result
text, not from the landing page. Re-check before use.
**Convention:** "verified" means a search result showed the item. "Not verified" means I could not confirm it,
and any detail given is from background knowledge only.

## 0. Data-policy flags (read first)

CLAUDE.md allows only open/public literature and properly licensed public benchmark data. It
**forbids "operational facility data"**. Three access classes matter here:

| Class | Usable? |
|---|---|
| **A. Open**: an open government report (NUREG, ANL/INL report on OSTI/UNT) or an open-licence dataset (CC BY) | Yes, with provenance |
| **B. Public literature figures/tables** (journal papers, digitised) | Yes as "public literature data". Record the digitisation steps (CLAUDE.md data provenance) |
| **C. NEA Data Bank restricted / benchmark-participant NDA** | **No.** Do not assume usable. The *public specification reports* are usable, but their numeric databases are not |
| **D. Measured data from an operating power plant** (Peach Bottom, Ringhals, Oskarshamn) | **Maintainer decision needed.** Even where it is printed in a public NEA report, it is measured operational plant data, which DATA_POLICY may exclude. Do not build on it without explicit approval |

## Part A: experimental datasets

### A1. Subcooled and bulk void fraction

| Dataset | What, geometry, conditions | Reference | Where the data is, and access | Validates |
|---|---|---|---|---|
| **Bartolomei** | Area-averaged void along heated vertical tubes in subcooled and low-quality boiling, water, roughly 3–15 MPa (ranges from background knowledge; not verified) | Bartolomei & Chanturiya, *Thermal Engineering* 14, 123–128 (1967; one citing list says 1963, which is inconsistent). Bartolomei et al., *Thermal Engineering* 29(3) (1982), "An experimental investigation of true volumetric vapour content with subcooled boiling in tubes". **DOI: not verified** | **Class B.** The journal is a Russian translation journal behind a paywall. The case conditions are widely reproduced, for example in the ANSYS Fluent verification manual case VMFL039 (a commercial vendor document, so cite the original instead). Data points have to be digitised from the paper figures | Onset of significant void (Saha-Zuber), subcooled-boiling void (Levy/Lahey/Rouhani), wall-heat-flux partitioning, drift flux. **The standard first subcooled-void case** |
| **Christensen (1961)** | ANL power-to-void experiment, rectangular electrically heated channel, heated length 127 cm, 2.7–6.9 MPa (NEA SET entry). Christensen's subcooled steady void data is commonly cited from this work | ANL-6385, "Power-to-void transfer functions": **the report number and authorship were NOT verified.** The NEA SET facility entry (oecd-nea.org/tiethysweb/facility/set/177) does not name Christensen | ANL reports from this era are usually open on OSTI or UNT. **ANL-6385 was not found online: not verified** | Subcooled void, void-quality relation, power-to-void dynamics (a direct test for density-wave stability models) |
| **Marchaterre** | Natural- and forced-circulation boiling studies at ANL | ANL-5735, Marchaterre, Petrick et al., May 1960 (found as a scan on UNT Digital Library). **ANL-5522 not verified** | **Class A, probably** (open UNT/OSTI scan). The table layout is not checked | Natural-circulation void, slip/drift flux at low pressure, two-phase friction |
| **Egen** | Void in subcooled boiling, about 2000 psia (background knowledge) | BMI-1163 (Battelle): **not verified** | Not found | Subcooled void at high pressure |
| **Jiji–Clark** | Bubble boundary layer, forced-convection subcooled boiling in a rectangular channel | Jiji & Clark, *J. Heat Transfer* (1964): **not verified.** A related 1962 Jiji report (U. Michigan Deep Blue) is open | Deep Blue report: **Class A** (open repository; licence not checked) | Onset of nucleate boiling, bubble-layer models |
| **FRIGG loop** (Nylund) | Full-scale 36-rod Marviken element, uniform and non-uniform radial heat flux, gamma-transmission void, BWR pressure | Nylund et al., FRIGG-2, AB Atomenergi/ASEA 1968, and FRIGG-3. The void series is Skaug et al., FRIGG-PM-15 (NEA SET entry 93) | **Access not verified.** These are Swedish industrial reports. FRIGG void data does appear in later open papers (for example, Griffiths et al. 2014 used it to assess Chexal-Lellouche) | Bundle-average void, drift flux in rod bundles |
| **NUPEC BWR void** (precursor of BFBT) | X-ray CT subchannel void in a full-size 8x8 BWR mock-up, 1987–1995 | Inoue et al., *J. Nucl. Sci. Technol.* 32(7), 629–640 (1995), hosted on the NEA BFBT pages. Also *Nucl. Technol.* 112(3), 388–400 (1995). **DOI not verified** | **Class B.** Figures in the open papers can be digitised | Subchannel void, bundle void |
| **OECD/NRC BFBT** | NUPEC full-size 8x8 BWR bundles: void (X-ray CT and densitometer), critical power, pressure drop, at BWR conditions | Specification NEA/NSC/DOC(2005)5 (a public PDF on the NEA site). Vol. II UA/SA report (public PDF) | **Class C, restricted.** NEA-1731 database package: "Tested restricted", NDA, participants only. The specifications are also marked "RESTRICTED DISTRIBUTION – Only for BENCHMARK PARTICIPANTS". **Do not use the numeric data** | Subchannel void, critical power (BWR CHF/dryout), bundle Δp |
| **OECD/NRC PSBT** | NUPEC PWR subchannel and 5x5 bundle tests: X-ray CT void (chordal, at 3 elevations) and DNB | Vol. I, NEA/NSC/DOC(2012)1. Vol. III (DNB), 2015 (NEA/NSC/R(2015)4, filename seen) | **Class C.** NEA-1849 database (Excel) is participants-only. The public Vol. I report may contain tabulated results, **not verified** | Subcooled void in subchannels, DNB |
| **DEBORA** (CEA Grenoble) | R12 boiling in a 19.2 mm heated pipe (3.5 m), 14 and 26 bar (a water-similar density ratio), local void, bubble diameter, velocity | Search results say a 2026 *Int. J. Heat Mass Transfer* paper (HAL hal-05571906) publishes the **full dataset under CC BY 4.0**. **I did not open it; DOI and repository not verified** | **Class A if confirmed.** This could be the best open local-boiling dataset | Multidimensional (CFD) wall boiling, interfacial area, subcooled void. For 1-D codes, the area-averaged void only |
| **Stanford Multiphase / Delhaye** | Not researched in depth. Delhaye, Maugin & Ochterbeck (2004, IJHMT) is a void-prediction model checked against DEBORA and Bartolomei | **Not verified** | **Not verified** | — |

### A2. Dryout / CHF

| Dataset | What | Reference | Access | Validates |
|---|---|---|---|---|
| **Groeneveld 2006 CHF look-up table (LUT)** | CHF at 24 pressures × 20 mass fluxes × 23 qualities, 8 mm tube, water. More than 30,000 points behind it | Groeneveld et al., "The 2006 CHF look-up table", *Nucl. Eng. Des.* 237 (2007) 1909–1922. **DOI not verified** | The LUT is **tabulated in the paper (Class B)**. **The underlying data is open: NUREG/KM-0011 (NRC, Jan 2019, D.C. Groeneveld)** compiles more than 25,000 CHF points from 62 datasets, with an attachment "CHF Data for Groeneveld 2006 LUT" (6.1 MB PDF). **Class A, US government open report.** Column layout not checked | CHF/dryout models, LUT interpolation, diameter correction |
| **Bennett (AERE Harwell, 1967)** | Dryout in vertical uniformly heated tubes, 12.62 mm ID, heated lengths 3.66 and 5.56 m, G = 393–5235 kg/m²s. The 224-case series is described in arXiv:2505.14701 | AERE-R 5373: **report number not verified** | UKAEA report, **open access not verified.** These data may be among the 62 sets in NUREG/KM-0011 (not verified) | Dryout location, post-dryout (Bennett is also the classic post-dryout wall-temperature set) |
| **Becker (KTH)** | Round-tube burnout, 30–200 bar. Later series used 10/15/25 mm tubes and 6 axial power shapes | KTH-NEL-14 (1971). Becker et al. 1983 and 1992 | **Not verified.** Possibly inside NUREG/KM-0011 | Dryout with non-uniform heating (a test of the boiling-length vs local-condition approach) |
| **MIT IR flow boiling** (Richenderfer, Bucci) | IR-thermography subcooled flow boiling to CHF, 1 atm, subcooling 10/25/50 K, G = 500–1000 kg/m²s | MIT CASL report (Richenderfer, Kossolapov, Seong, Bucci, Buongiorno 2017, "Approved for Public Release", on info.ornl.gov) | The report is **Class A**. **No open data release found** | Mechanistic heat-flux partitioning, CHF |
| **HZDR MORENA** | IR data, 3 and 5 K subcooled flow boiling | rodare.hzdr.de record 394/395 | **Restricted access.** Do not use | — |
| **TAMU; OECD/NEA "CHF/PDO"** | — | **Not verified** | **Not verified** | — |

### A3. Critical flow and blowdown (both already in this repo)

| Dataset | Status in repo | Reference / access |
|---|---|---|
| **Edwards–O'Brien pipe blowdown** | **Already used**: `crates/tampines-steam-tables/tests/edwards_blowdown.rs`, `crates/tampines/src/multiphase_1d/edwards_tests.rs`, CI smoke `crates/outram-park-digital-twin-engine/tests/ci_smoke/edwards.rs`, history in `crates/tampines-steam-tables/docs/edwards_blowdown_solver_debugging.md` | *J. Br. Nucl. Energy Soc.* 9, 125 (1970). **Note:** one citing list gives O'Brien as sole author, while others cite "Edwards & O'Brien". Authorship is inconsistent across sources. Class B |
| **Marviken CFT** | **Already used**: `.../converging_diverging_nozzles/tests/marviken_tests.rs`, `crates/tampines/src/multiphase_1d/marviken_tests.rs`. The literature is in kovan (`nureg-cr-2671-marviken`) | NUREG/CR-2671 summary report: **Class A**. Docs/reactor-scoping/bwr.md calls this "the only genuinely validated two-phase physics in the workspace" |
| Moody (1975, NEDO-21052) and Zaloudek (1961) | **Already used** in the same nozzle test folder | — |
| **Super Moby Dick** (CEA, 1979–82) | Not in repo | Steady critical flow through nozzles, medium to high pressure. "Long nozzle" test section: 66.7 mm inlet, 20.13 mm throat pipe, 7° diffuser. NEA SET facility entries 12 and 33, NEA package CSNI1017 (**access not verified**). Used in the literature to compare Moody, Fauske, Henry-Fauske and DEM; DEM agrees best. The Réocreux link is from background knowledge, not verified |

### A4. Integral, stability and plant data

| Dataset | What | Access |
|---|---|---|
| **Peach Bottom 2 turbine trip** (NEA BWRTT) | Measured BWR/4 turbine-trip transient (TT2, 1977), coupled 3-D kinetics and T-H | Specification NEA/NSC/DOC(2001)1 (a public PDF on the NEA site). Package NEA-1640 (**access not verified**, fetch failed). **Class D: operational plant data, maintainer decision** |
| **Ringhals-1 stability** | Measured stability (decay ratio) data, cycles 14–17 | Specification NEA/NSC/DOC(94)15, results NEA/NSC/DOC(96)22. Package NEA-1454 is **restricted to participants**. **Classes C and D** |
| **Oskarshamn-2 stability** (1999 event) | Diverging power oscillation, decay ratio above 1 | **OKG NDA, participants only.** NEA document number not verified. **Classes C and D** |
| **Purdue PUMA / PUMA-E** | Scaled SBWR/ESBWR integral facility: MSLB, GDLB, BDLB, FWLB | Data held in the NRC databank and used for TRACE/MELCOR assessment. NUREG/CR number **not verified**, and whether the numeric data is open is **not verified** |
| **KATHY** | Framatome (AREVA) thermal-hydraulic loop, Karlstein, used for BWR stability and critical power. Background knowledge only | **Industrial proprietary (not verified). Do not assume usable** |
| **ATLAS** (KAERI) | PWR integral test. OECD-ATLAS projects | Not BWR. Joint-project data is restricted (not verified) |
| **NRC NUREG/IA** (TRACE/RELAP assessments) | International assessment reports. They often tabulate or plot facility data next to code results | NRC public reports, **Class A** for what they print. Individual report numbers **not verified** in this pass |

## Part B: BWR system-level models, and open sources

**Workspace note (search-before-building):** `docs/reactor-scoping/bwr.md` already lists
Zuber-Findlay algebraic slip and mixture closures in `crates/outram-foam-multiphase/src/drift_flux.rs`,
and a 1-D four-equation drift-flux marcher (gravity, wall friction, drift momentum flux, vapour relaxation) in
`crates/tampines/src/multiphase_1d/drift_flux.rs`, plus two-fluid code. The models below should go
**into those modules**, not into new ones.

### B1. Drift flux / void-quality

| Model | Source | Open? | Validity |
|---|---|---|---|
| Zuber–Findlay (1965) | *J. Heat Transfer* 87(4), 453–468, **DOI 10.1115/1.3689137 (verified)** | Paywalled journal (Class B) | General framework (C0, Vgj). Its constants are flow-regime dependent |
| Ishii (1977) | ANL-77-47, Oct 1977. **Open scan on UNT Digital Library (verified)** | **Class A** | Flow-regime C0/Vgj for bubbly, slug, churn and annular flow. Medium pipes (roughly D/Laplace length ≤ 30) |
| Hibiki–Ishii (2003) | *IJHMT* 46, 4935–4948, **DOI 10.1016/S0017-9310(03)00322-3 (verified via erratum record)**. Erratum DOI 10.1016/j.ijheatmasstransfer.2004.12.001 | Class B | Updated regime correlations, including large pipes |
| Chexal–Lellouche (EPRI) | EPRI NSAC/139 (1991), with a 1996 revision. **Access: EPRI report, open availability not verified (likely licensed)** | Probably not open. It is used in RETRAN-3D and RELAP5. If the RELAP5-3D or TRACE manual prints the full equations, porting from that open manual is an option, after a licence check | Full-range (0–1 void), counter-current. It has been criticised for compensating errors (Griffiths et al. 2014) |
| Bestion | CATHARE drift/interfacial-friction closures. **Not verified** | — | — |
| Void–quality (homogeneous, slip-ratio, and Levy / Saha-Zuber subcooled profile) | Covered in the Lahey & Moody textbook and the TRACE/RELAP manuals | Manuals: Class A | — |

### B2. Two-phase friction multipliers

| Model | Citation | Status |
|---|---|---|
| Lockhart–Martinelli (1949) | *Chem. Eng. Prog.* 45, 39–48 (from background knowledge, **not verified**) | Class B. Low pressure, adiabatic |
| Martinelli–Nelson (1948) | *Trans. ASME* 70, 695 (**not verified**) | Steam-water, forced circulation. The BWR classic |
| Friedel (1979) | European Two-Phase Flow Group Meeting, Ispra, paper E2 (verified as a citation). The journal version (3R International 18, 485–49x) has inconsistent page ranges | Wide-range. Used in many system codes |
| Chisholm (1967 / B-coefficient 1973) | **Not verified** | — |
| Müller-Steinhagen & Heck (1986) | *Chem. Eng. Process.* 20(6), 297–308 (verified). **DOI not verified** | Simple. Performs well in heated-tube assessments |

### B3. Natural circulation and density-wave stability

- **Lahey & Moody, *The Thermal-Hydraulics of a Boiling Water Nuclear Reactor*, ANS 1977**, ISBN 0894480103 (1st edition, verified). **A 2nd edition (1993) was NOT verified.** This is a commercial book: cite it, and do not copy tables wholesale.
- **March-Leuba reduced-order model:** the core paper is in *Nuclear Technology* (1986, five-equation linear model). ORNL reports found: ORNL/TM-8546 (March-Leuba & Otaduy 1983, LAPUR-IV) and NUREG/CR-5605 (LAPUR benchmark against in-phase and out-of-phase tests, 1990). A dedicated ORNL report on the ROM was **not verified**. ORNL/NUREG reports are normally open (OSTI/NRC ADAMS). Later open ROM extensions exist (Dokhane/PSI, TU Dresden, Chalmers).

### B4. Critical flow

| Model | Source | Status |
|---|---|---|
| HEM | Already implemented and validated (Zaloudek, Moody, Marviken tests) | — |
| Moody (1965) slip-equilibrium | *J. Heat Transfer* 87 (1965): **not verified in this pass.** The repo already cites Moody (1975) NEDO-21052 | — |
| Henry–Fauske (1971) | *J. Heat Transfer* 93, 179–187. DOI 10.1115/1.3449782 (taken from a citing reference list, not resolved independently) | Non-equilibrium, short nozzles |
| Delayed Equilibrium Model (DEM, UCLouvain, Seynhaeve et al.) | **Not verified.** The search returned nothing relevant. Super Moby Dick comparisons report DEM agreeing best | — |

### B5. Open code manuals (closure-library sources)

| Manual | Status |
|---|---|
| **TRACE V5.0 Theory Manual** ("Field Equations, Solution Methods, and Physical Models") | NRC ADAMS **ML071000097** (draft, verified, public). The user-supplied ML120060218 was **not verified**. Reviewer comments are in public ADAMS ML0907/ML090770758–61 |
| **RELAP5/MOD3 Code Manual Vol. 4, Models and Correlations** | NUREG/CR-5535 (INEL-95/0174), indexed on OSTI (**Class A**) |
| **RELAP5-3D manuals** | Vol. I INEEL-EXT-98-00834 Rev 2.3 (2005) exists. The 2015-era manuals appear as INL reports. **The public availability of the current Vol. IV (INL/MIS-15-36723) was not verified**, and the "MIS" designation suggests limited distribution |
| **MOOSE THM** | Open source (MOOSE licence: LGPL-2.1, from background knowledge, so check it). **The THM flow model is single-phase only** (`Closures1PhaseTHM`). The two-phase LWR closures are in **RELAP-7**, which is not open source (not verified). THM therefore gives **no two-phase BWR closures** to port |

## Ranked: first V&V cases to build (open and tabulated first)

1. **Groeneveld 2006 CHF LUT vs NUREG/KM-0011 data** (Class A, US government open). A pure closure verification: implement the LUT and reproduce its published statistics against the open data points. Dryout is the BWR thermal limit.
2. **Ishii (ANL-77-47) drift-flux closures** in the existing `drift_flux.rs`, verified against Ishii's own regime equations and the Zuber-Findlay limits (Class A). The cheapest win, because it extends code that already exists.
3. **Bartolomei subcooled void** (Class B, digitised). Run it uncalibrated with Saha-Zuber, a Levy-type profile and the drift-flux marcher with wall heat. This is the universal first subcooled-void case. Record the digitisation provenance.
4. **DEBORA**, area-averaged void only, **if the CC BY 4.0 release is confirmed** (Class A). It cross-checks case 3 with a second fluid (R12), which tests fluid-to-fluid scaling.
5. **Super Moby Dick critical flow**, extending the existing Marviken/Moody suite to Henry-Fauske and DEM. Requires the access to be confirmed and the data to be tabulated in an open paper.
6. **Two-phase friction multipliers** (Martinelli-Nelson, Friedel, Müller-Steinhagen-Heck) against one another and against open NUREG/IA tabulations.
7. **March-Leuba ROM**: an analytical, self-consistent verification of the decay ratio from the published linear model. No plant data is needed.
8. **Marchaterre ANL-5735 natural circulation**, once the tables are confirmed in the UNT scan.

**Do not build on these without the maintainer:** BFBT, PSBT numeric databases (Class C);
Peach Bottom TT, Ringhals-1, Oskarshamn-2 (Classes C and D); KATHY; MORENA.
