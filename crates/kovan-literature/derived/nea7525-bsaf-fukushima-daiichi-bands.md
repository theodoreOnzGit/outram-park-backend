# Fukushima Daiichi units 1–3: BSAF Phase II code-spread bands (NEA No. 7525)

The **numeric** results of the OECD/NEA Benchmark Study of the Accident at the
Fukushima Daiichi Nuclear Power Station (BSAF), Phase II: every participant's
predicted event time, failure mode, debris mass and release, as the summary
report tabulates them, plus the bands its figures and narrative give.

This is the **validation target** for OUTRAM PARK's Fukushima accident
simulator (gh:#1028, `crates/sembawang`). It is a *code-to-code* spread, not a
measurement: BSAF is fourteen organisations running eight severe-accident codes
against the same accident, and the width of each band is the honest statement
of how well the state of the art constrains that quantity. Where the report
also gives a *measured* value, this record says so explicitly — those are the
only entries that can validate anything.

**What kind of numbers these are:**
- The **event times, failure modes, debris masses and integral releases** are
  each participant's **code prediction**, not an observation.
- The **RPV / drywell pressure histories and CAMS dose rates** have measured
  counterparts, recorded here as such.
- The **initial core inventories** are **benchmark specifications**, supplied
  to every participant by the co-ordinator.
- Several quantities (hydrogen mass, concrete erosion depth, release
  fractions) are published **only as figures**. Those are marked
  *figure-read* and carry a reading uncertainty; they must not be quoted to
  more precision than the mark.

---

## Provenance

| Field | Value |
|---|---|
| Source | OECD Nuclear Energy Agency (2021), *Benchmark Study of the Accident at the Fukushima Daiichi Nuclear Power Plant (BSAF Project) — Phase II: Summary Report*, NEA No. 7525, 68 pp. Operating agent: Institute of Applied Energy (IAE), Japan |
| Sections used | Table 1.1 (p.16); Ch. 2 and Tables 2.1–2.2, Figs 2.1–2.8 (pp.17–23); Ch. 3 and Table 3.1, Figs 3.1–3.8 (pp.25–32); Ch. 4 and Tables 4.1–4.2, Figs 4.1–4.8 (pp.35–42); Ch. 5 and Table 5.1 (pp.43–47); Ch. 7 (pp.57–62); Executive summary (pp.11–14) |
| Copy held | the maintainer's private repository, `propreitrary-literature-theodore`, `reports/nea-7525-bsaf-fukushima-daiichi-summary-report.pdf` (added 2026-10-27) |
| Access tier | **Proprietary.** "© OECD 2021 … You can copy, download or print OECD content for your own use, and you can include excerpts … provided that suitable acknowledgement of the OECD as source and copyright owner is given. All requests for public or commercial use and translation rights should be submitted to neapub@oecd-nea.org" (copyright page). Permission for public use has not been sought, so **the PDF and its extracted full text stay in the private repository**. This record holds facts, figures and numbers — not copyrightable — with acknowledgement, which those terms allow |
| Processing | Tables transcribed cell-for-cell from the rendered pages by an AI agent, 2026-10-27. Figure values read **by eye** off the plotted curves and marked *figure-read*. No digitiser was used. **Not reviewed by a human.** |

**Acknowledgement.** Source and copyright owner: OECD Nuclear Energy Agency,
*Benchmark Study of the Accident at the Fukushima Daiichi Nuclear Power Plant*,
NEA No. 7525, © OECD 2021.

---

## 0. Participants and codes (Table 1.1, p.16)

| # | Country | Organisation | Code | Units analysed |
|---|---|---|---|---|
| 1 | Canada | Canadian Nuclear Laboratories (CNL) | MELCOR | 2 |
| 2 | Finland | Technical Research Centre of Finland (VTT) | MELCOR 2.2 | 1–3 |
| 3 | France | Alternative Energies and Atomic Energy Commission (CEA) | TOLBIAC | 1 (MCCI) |
| 4 | France | Radioprotection and Nuclear Safety Institute (IRSN) | ASTEC V2.0 rev3 p1 | 1–3 |
| 5 | Germany | GRS | GRS Code | inverse analysis |
| 6 | Japan | Central Research Institute of Electric Power Industry (CRIEPI) | MAAP 5.01 | 3 |
| 7 | Japan | Japan Atomic Energy Agency (JAEA) | THALES 2 | 1–3 |
| 8 | Japan | Institute of Applied Energy (IAE) | SAMPSON-B 1.4 beta | 1–3 |
| 9 | Japan | Nuclear Regulation Authority (NRA/S/NRA/R) | MELCOR 2.1 | 1–3 |
| 10 | Korea | Korea Atomic Energy Research Institute (KAERI) | MELCOR 1.8.6 / MELCOR 2.1 | 1 / 2 |
| 11 | Russia | Nuclear Safety Institute, Russian Academy of Sciences (IBRAE)/Rosatom | SOCRAT/V3 | 1 |
| 12 | Spain | Nuclear Security Council (CSN) / CIEMAT | MELCOR 2.1-4803 | 1 |
| 13 | Switzerland | Paul Scherrer Institute (PSI) | MELCOR 2.1_4203 | 3 |
| 14 | United States | NRC / DOE / Sandia National Laboratories (SNL) | MELCOR 2.1-5864; HYSPLIT | 1–3; forward analysis |

Phase I ran Nov 2012 – Dec 2014 (first six days). Phase II ran Apr 2015 –
Mar 2018 and extends the analysis to **21 days (500 hours)** after the
accident. 21 organisations in 11 countries were involved in the project
overall; the 14 above contributed Phase II analyses.

**Note on code lineage.** Eight of the fourteen entries are MELCOR variants.
The band widths below are therefore **not** fourteen independent models: a
quantity on which MELCOR versions agree is less well constrained than its
spread suggests.

---

## 1. Unit 1

### 1.1 Timeline anchors (narrative, pp.11, 17, 19)

| Event | Value | Kind |
|---|---|---|
| Earthquake / scram | 11 Mar 2011, 14:46 JST | observed |
| RPV self-depressurisation | between **5 and 12 h** from scram | **measured** — RCS pressure read only twice by operators in this window |
| Core damage commenced | **5–12 h** from shutdown | inferred across all analyses |
| Most predicted RCS leakages | **3–4 h**, regardless of assumed path | code prediction |
| Lower-head debris attack | **about 7–15 h** | code prediction (exec. summary wording) |
| S/C manually vented | **≈24 h** | observed |
| S/C vent closure | **24.68 h** | boundary condition |
| Reactor-building hydrogen explosion | **24.83 h** | observed |
| PCV pressure trend reverses, then rises again | falls from ~50 h for >200 h; rises sharply from **270 h** | **measured** (matched by assuming increased alternative water injection) |
| MCCI still ongoing at end of simulation | **500 h**, in almost all calculations | code prediction — *no plant evidence confirms this* |

Unit 1 was the first reactor to suffer severe damage. For the first ten hours
operators were largely blind to the reactor's state; no operator control of RPV
pressure, and no active control of PCV pressure for the first 24 hours.

### 1.2 RCS leak / failure time and path (Table 2.1, p.18) — hours from scram

| Path | CIEMAT | IAE | IBRAE | IRSN | JAEA | KAERI | NRA | SNL | VTT |
|---|---|---|---|---|---|---|---|---|---|
| SRM/TIP pipe | 1.0 | – | 3.6 | – | 2.47 | – | 4.1 | – | – |
| SRV gasket | 4.61 | 4.1 | 10.9 | 3.68 | – | – | 4.2 | – | – |
| PLR pump leak | – | – | – | – | – | – | – | – | 1.0 |
| MSL failure | – | – | – | – | – | **5.0** | – | **6.1** | – |

SRM = source range monitor; TIP = transverse instrumentation probe;
SRV = safety relief valve; PLR = primary loop recirculation; MSL = main steam
line. CIEMAT, IBRAE and IRSN each assume **two** leakage pathways. KAERI and
SNL are the only teams modelling a **main steam line failure**, giving an
immediate depressurisation rather than a gradual loss.

### 1.3 RPV lower-head failure and debris mass (Table 2.2, p.18)

| | CEA | CIEMAT | IAE | IBRAE | IRSN | JAEA | KAERI | NRA | SNL | VTT |
|---|---|---|---|---|---|---|---|---|---|---|
| **Lower-plenum failure (h)** | – | 11.58 | 14.97 | 15.10 | 17.16 | **7.45** | 13.50 | 13.6 | 12.5 | 11.42 |
| **Failure mode** | – | Melt through | Penetration failure | Melt through | Creep | Melt through | Penetration failure | Melt through | User specified | Penetration failure |
| **Total mass in pedestal (t)** | 39 ᵃ | 129 | 148 | 196 | 282 ᵇ | 130 | 136 | 115 | 154 | 111 |

ᵃ CEA's value counts only the debris in the two sumps — TOLBIAC considers only
the sumps, not the mass above the pedestal and drywell floor.
ᵇ IRSN's value includes all in-core materials **and the RPV lower head itself**;
it is a modelling hypothesis.

**Band: 7.45 – 17.16 h**, nine predictions, median ≈ 13.5 h. Excluding the two
geometry-incomparable debris masses (CEA, IRSN), the pedestal mass band is
**111 – 196 t**; the narrative states most calculations give **120–190 t** of
degraded core released at lower-head failure.

CIEMAT and VTT put the failure at the time of the largest measured PCV pressure
(≈11.5 h). IRSN, IBRAE and IAE instead attribute that pressure rise to debris
slumping into the lower head and fail the vessel a few hours later. JAEA
associates it with MCCI onset and fails earliest.

### 1.4 In-vessel hydrogen generation (Fig. 2.2, p.19) — *figure-read*

Integrated H₂ at 25 h from scram, read by eye off the plotted plateaux
(±~30 kg):

| Band | Value |
|---|---|
| Lowest (JAEA) | **≈ 400 kg** |
| Bulk of participants | **≈ 600 – 1 000 kg** |
| Highest (IBRAE) | **≈ 1 310 kg** |

Sharp generation begins at **3–5 h** in all calculations and is essentially
complete by 10–15 h, with a small second step near 15 h in several.

### 1.5 Containment pressure (Figs 2.1b, 2.3a) — has measured counterpart

- Drywell pressure over 0–25 h: measurements (two points plotted) against nine
  calculations; calculated peaks reach **≈0.8–1.0 MPa(abs)** around 10–16 h,
  settling to a common **≈0.75–0.8 MPa** plateau by 20 h. *figure-read.*
- Over 0–500 h the measured trace is dense and well reproduced: a fall from
  ≈0.8 MPa to **≈0.15–0.2 MPa** by 250 h, a rise to ≈0.4 MPa near 300 h, then
  a slow decline. All calculations impose a **variable-cross-section
  containment leak from 50 h on** to follow it — except JAEA, which postulates
  venting from the S/C instead. *figure-read.*
- Three participants attribute the leak to a penetration; the other five to
  **MCCI erosion of the drywell liner**, consistent with TEPCO's observation of
  water leaking into the torus room from the sand-cushion region outside the
  PCV liner (TEPCO, 2013).

### 1.6 Alternative water injection (Fig. 2.3b)

All calculations that captured the 270 h pressure change agree the effective
injection rate is **below 4.5 kg/s**, but differ on the profile. Prior to this
time the injection is **taken to be 0**.

### 1.7 Concrete erosion depth (Fig. 2.4, p.20) — *figure-read*

Depth at 500 h, origin at the bottom of the sump (±~0.2 m):

| Band | Value |
|---|---|
| Lowest (KAERI) | **≈ 1.2 m** |
| Bulk | **≈ 2.5 – 3.2 m** |
| CEA (TOLBIAC, ends at 300 h) | **≈ 4.0 m** |
| Highest (JAEA) | **≈ 6.9 m** |

Most institutes model instantaneous corium spreading in the pedestal or sump
(2.5–3 m) as a code default; only IAE calculates the spreading process itself.

### 1.8 Fission-product inventory and release

**Benchmark-specified initial core inventory** (p.20): **Cs 154.0 kg**,
**I 11.98 kg**.

| Quantity | Value | Kind |
|---|---|---|
| Release **from fuel** (Xe, Cs, I), asymptotic | generally **> 80 %** of initial inventory; most reach 0.9–1.0 | code prediction, *figure-read* (Fig. 2.5) |
| Onset of release from fuel | **3–6 h** | *figure-read* |
| Airborne fraction in PCV | typically **< 10 %** | code prediction |
| Cs and I scrubbed into S/C water by 35 h | **≈10 – 80 %** | code prediction, *figure-read* (Fig. 2.6c,d) |
| Cs deposited on D/W structures | ≈0.02 – 0.7 (JAEA highest) | *figure-read* (Fig. 2.6a) |
| First release to **environment** | **≈10 h**, when the PCV head flange is predicted to start leaking | code prediction, consistent across almost all calculations |
| Integral release after PCV venting at **24.7 h** | **6×10¹³ – 5×10¹⁵ Bq** (about two orders of magnitude spread) | code prediction vs GRS/WSPEEDI inverse estimate |
| Cs released to environment | most calculations **< 2 %**; best fit to the inverse calculations is **0.5 – 1.0 %** | code prediction / inverse analysis |
| I released to environment | most calculations **≈5 %**; best fit to inverse is **≈1 %** | code prediction / inverse analysis |

**CAMS dose rate (Fig. 2.7)** — measured, two systems each in D/W and S/C.
The analyses generally **over-predict** the D/W dose rate, especially early,
and **under-predict** the S/C dose rate. Measured D/W settles near 40 Sv/h at
300–450 h; measured S/C near 20–45 Sv/h. *figure-read.*

---

## 2. Unit 2

### 2.1 Timeline anchors (narrative, pp.12, 25–28)

| Event | Value | Kind |
|---|---|---|
| RCIC operation, uncontrolled "self-regulating" mode | **≈70 h** | observed; mechanism still not fully understood |
| RPV water level above maximum measurable | from 11 Mar ≈22:00 | **measured** |
| RCIC failure | **≈67–70 h** | observed |
| Core heat-up and degradation start | **≈75 h**, shortly after operators depressurised the RPV | code consensus |
| "RPV 3 pressure peaks" period | **75 – 85 h** | **measured** and reproduced |
| SRV reopening | **≈78.5 h** | boundary condition |
| PCV pressure maximum, close to **0.8 MPa** | **≈80.7 h** | **measured** |
| Fast PCV pressure decrease | **≈89 h** (exact timing unknown — gap in the data record) | **measured** |
| Alternative water injection available | from **≈75 h** at the earliest | observed |

The PCV pressure rise during RCIC operation was **lower than expected** for the
cumulative decay heat. Reproducing it required **thermal stratification of the
suppression pool** and **extra heat loss from partial seawater flooding of the
torus room** by the tsunami. *"None of the submitted results using lumped
parameter codes were able to properly match this phenomenon."*

### 2.2 Lower-head and containment failure (Table 3.1, p.28)

| | CNL | CRIEPI | IAE | IRSN | JAEA | KAERI | NRA | SNL | VTT |
|---|---|---|---|---|---|---|---|---|---|
| **LH failure time (h)** | 104.57 | 129.0 | 93.36 | 95.32 | 92.0 | **441.19** | 85.8 | 83.0 | – |
| **LH failure mode** | Penetration failure | Penetration ejection | RPV creep | Penetration failure | Penetration failure | Penetration failure | Penetration failure | Penetration failure | **No failure** |
| **PCV failure (h)** | 89.0 | 88.54 | 88.55 | 88.57 | 87.0 / 89.2 | 88.89 | N/A | N/A | 89.0 |
| **PCV failure mode** | Head flange leakage | Head flange leakage | Head flange leakage | X-6 penetration leakage | X-6 penetration / head flange | Head flange leakage | N/A | N/A | Head flange leakage |
| **S/C water leakage (h)** | 89.0 | N/A | N/A | N/A | 88.0 | N/A | N/A | N/A | N/A |
| **Total mass in pedestal (t)** | 78 | 399 ᶜ | 35 | 16 | 0 | 186 | 157 | 111 | 0 |

ᶜ **Internal inconsistency, flagged.** Table 3.1 gives CRIEPI 399 t and NRA
157 t, but the text on p.29 states the debris released into the pedestal
"varies from 16 tonnes for the IRSN up to **161 tonnes** for the NRA", and the
executive summary says **16–161 tonnes**. The 399 t entry carries a superscript
that resolves to the SNL institutional disclaimer footnote, not a data note, so
its meaning is unexplained. **Do not use 399 t without checking the full
Phase II report.** Use the narrative band 16–161 t as the defensible figure.

**Band (excluding KAERI's outlier and VTT's no-failure): 83.0 – 129.0 h.**
Most calculations cluster **83–105 h**.

### 2.3 Hydrogen (Fig. 3.2c, p.27) — *figure-read*

Integrated H₂ at 85 h, read by eye (±~30 kg): **≈400 – 950 kg**.

The report draws a direct link: the three institutes predicting **more than
800 kg** (IAE, IRSN, NRA) were the ones that came close to reproducing the
measured PCV pressurisation. Those predicting less **did not** reproduce it.
This is the clearest hydrogen constraint in the whole study.

### 2.4 Fission products

| Quantity | Value |
|---|---|
| Release from fuel (Xe, I) | generally **> 80 %** of initial mass, starting ≈75 h, almost complete by ≈82 h (end of the second RPV pressure peak). KAERI alone predicts a much slower release |
| Cs/I deposited on D/W structures | most **< 3 %**; IAE ≈23 % of I, CNL ≈7 % |
| Cs/I retained in S/C pool water | **< 10 % (KAERI) to ≈75 % (IRSN)** in the long term; JAEA ≈90 % during core degradation |
| **Cs released to environment** | **< 0.1 %** (IRSN, IAE); ≈0.1 % (VTT, JAEA); **1.5 – 5 %** (all others) |
| **I released to environment** | **< 0.5 %** (IRSN, NRA); 2–5 % (JAE, KAERI, SNL); 9 % (CNL); ≈17 % (IAE) |
| Noble gases | almost all released through PCV leakage within ≈10 h of the 89 h PCV failure |

**CAMS dose rate (Fig. 3.6)** — **measured**: from ≈75 h, D/W CAMS rose from
**10 → ≈100 Sv/h** and S/C CAMS from **3 → ≈10 Sv/h**. The S/C peak at ≈80 h is
**over-predicted by most calculations by a factor of 4–5**, which the report
judges to be within the uncertainty of the dose-conversion methodology. A
further measured D/W peak at **≈95 h** may indicate RPV failure.

---

## 3. Unit 3

### 3.1 Timeline anchors (narrative, pp.13, 35–37)

| Event | Value | Kind |
|---|---|---|
| RCIC cooling | first **≈20 h**; stopped automatically on high S/C pressure | observed |
| HPCI start | ≈1 h after RCIC stop (**≈21 h**) | observed |
| HPCI performance begins to degrade | **≈30 h** (assumed by the calculations) | modelling assumption |
| HPCI manually stopped | **36 h** | observed |
| Period with **no** coolant injection | **≈10 h** after HPCI stop; water level drops below bottom of active fuel | observed/inferred |
| ADS actuation (6 SRVs), believed accidental | **≈42 h**; PCV rises to **0.6 MPa** | inferred |
| Venting from S/C gas space | **≈42.5 h** | observed |
| Only other TEPCO-confirmed vent | **45 h** (TEPCO, 2017a) | observed |
| Reactor-building hydrogen explosion | **68 h** | observed |
| Unit 4 reactor-building explosion | ≈19 h after unit 3's | observed |
| Containment pressure stays high until | **≈130 h** | **measured** |

Unit 3 had **DC power after the tsunami**, so a large amount of measured data
exists — water level, RPV pressure, PCV pressure and lower-head temperature
over long periods. It is the best-instrumented of the three.

### 3.2 RPV lower-head failure and debris mass (Table 4.1, p.37)

| | CRIEPI | IAE | IRSN | JAEA | NRA | PSI | SNL | VTT |
|---|---|---|---|---|---|---|---|---|
| **Time of failure (h)** | 101.95 | 55.20 | 55.37 | 46.48 | 49.40 | 73.05 | 58.03 | **43.30** |
| **Mode of failure** | Penetration failure | Creep failure | Creep failure | Vessel melt | Penetration failure | Penetration failure | User specified | Penetration failure |
| **Mass in pedestal (t)** | 244 | 105 | 51 | 188 | 65 | 21 | 205 | 224 |

**Band: 43.30 – 101.95 h**; most participants **43–58 h**, with two later
outliers at 73 h (PSI) and 102 h (CRIEPI). Pedestal mass band **21 – 244 t**.
TEPCO's 2017 containment investigations indicate the true debris mass is
**closer to the higher values** than the lower ones.

### 3.3 Assumed RPV → PCV leakage (Table 4.2, p.38) — hours

| Leak path | CRIEPI | IAE | IRSN | JAEA | NRA | PSI | SNL | VTT |
|---|---|---|---|---|---|---|---|---|
| MSL creep | | | | | | | **42.30** | |
| SRV leak | | 43.00 | | | 42.20 | | | |
| Pump seal | | | | **5.00** | | | | **6.33** |
| TIP leak | | | | 39.75 | 41.90 | | | |

JAEA and VTT assume an **early** RPV leak (5.00 and 6.33 h); all other leaks
are placed near the start of core degradation (≈40–43 h).

### 3.4 Hydrogen

- **In-vessel** (Fig. 4.2, p.36), *figure-read* at 50 h: **≈550 – 1 220 kg**,
  with generation starting between 37 h (IAE) and 45 h.
- **Ex-vessel** (Fig. 4.3c, p.37), *figure-read* at 100 h: up to
  **≈2 000 kg** (SNL), ≈1 500 kg (VTT), ≈900 kg (IRSN); others near zero.

### 3.5 Fission products

| Quantity | Value |
|---|---|
| Release from fuel | **90 – 100 %** of element inventory for most; ≈80 % for PSI, and for SNL in the case of I |
| Noble gases to atmosphere up to 68 h | **80 – 100 %** |
| **Cs to environment** | **3 – 5 %** (IRSN, NRA, VTT, released fast during the first containment vent); **< 0.5 %** for the majority |
| **I to environment** | **13 %** (IRSN); **4 – 9 %** (NRA, SNL, VTT); **≈2 % or less** (remaining four) |
| Cs/I in D/W at ≈60 h (from measured dose rate) | **≈5 %**, mainly by direct transport from the vessel |
| Concrete erosion at 500 h | *figure-read*: ≈1.1 m (CRIEPI) to ≈4.3 m (SNL) |

**Transport path not modelled:** TEPCO analysis indicates **20–35 %** of the
vented gas may have been diverted to the **unit 4** reactor building through
the common stack (it caused the unit 4 explosion). No BSAF calculation accounts
for this, so all unit 3 atmospheric releases are **over-stated** to that extent.

**Wind direction caveat:** during unit 3's main release events the dominant wind
was **towards the ocean**, so the inverse (backwards) estimates for unit 3 lean
on ocean-water sampling and are correspondingly less certain.

---

## 4. Integral Cs-137 release to atmosphere (Table 5.1, p.47) — PBq

| Participant | Code | Country | Unit 1 | Unit 2 | Unit 3 | Combined |
|---|---|---|---|---|---|---|
| CIEMAT | MELCOR 2.1-4803 | Spain | 5.6 | | | |
| CNL | MELCOR 2.1-6342 | Canada | | 8.1 | | |
| CRIEPI | MAAP 5.01 | Japan | | 6.7 | 0.8 | |
| **GRS** | **Backward calculation** | Germany | **0.79** | **10.7** | **1.6** | **13.1** |
| IAE | SAMPSON-B 1.4 beta | Japan | 4.4 | 0.0 ᵈ | 1.1 | 5.5 |
| IRSN | ASTEC V2.0 rev3 p1 | France | 0.1 | 0.1 | 43.1 | 43.2 |
| **JAEA** | **WSPEEDI** | Japan | | | | **14.5** ᵉ |
| KAERI | MELCOR 1.8.6 | Korea | 18.9 | 5.8 | 10.9 | 35.5 |
| NRC/DOE/SNL | MELCOR 2.1-5864 | United States | 2.8 | 12.3 | 10.6 | 25.7 |
| PSI | MELCOR 2.1-4206 | Switzerland | | | 0.2 | |

ᵈ "Little release predicted from the reactor building due to modelling deficit."
ᵉ See Katata et al., 2015.

- **Forward (severe-accident code) three-unit total: 5.5 – 43.2 PBq.**
- **Backwards / inverse reconstruction: 13.1 – 14.5 PBq** (GRS and WSPEEDI,
  two independent methods, in close agreement).

**The two bold rows are the closest thing to a measurement in this table** —
they are reconstructed from observed dose rates, ground deposition and ocean
sampling rather than predicted from plant state. A forward model landing
outside **13.1 – 14.5 PBq** for the three-unit total is disagreeing with the
observations, not merely with other codes. Per-unit, GRS attributes **0.79 PBq
to unit 1, 10.7 PBq to unit 2, and 1.6 PBq to unit 3** — assigning each unit's
contribution by assuming the release from the first failed unit stops when the
second starts.

Note how badly the forward codes split the total: IRSN puts **43.1 of its
43.2 PBq** on unit 3, while GRS puts the bulk on **unit 2**. The three-unit
total is far better constrained than any single unit's share.

**Time-dependent release.** The per-participant release-rate histories behind
this table (Figures 5.1–5.4) are digitised in
[`nea7525-bsaf-cs137-release-rates/`](nea7525-bsaf-cs137-release-rates) — the
curves a plume model needs, as opposed to the totals above. Read that folder's
quality table before using any series: a column-wise trace cannot integrate a
near-vertical spike, so the spike-dominated series (PSI on unit 3, IRSN on
unit 1) are lower bounds on magnitude and usable for shape only.

Deposition patterns (Fig. 5.5) were computed with HYSPLIT by SNL from each
participant's source term. Common features of the land-depositing cases:
**> 3 MBq/m² out to 10–20 km to the north-west**, and a peninsula of deposition
southward along the coastline. The north-west deposition was observed; the
onshore southern peninsula was **not**, though heavy contamination of the sand
along the seacoast to the south suggests deposition just offshore (Katata et
al., 2015).

---

## 5. Modelling gaps the study itself names (Ch. 7, pp.57–62)

Directly useful as a scoping list — these are where the state of the art is
known to be weak, so a new model is not obliged to do better, but must not
claim to:

1. **MCCI is over-predicted.** All codes except the dedicated MCCI code
   (TOLBIAC) predict extensive, unterminating molten-core-concrete interaction
   out to 500 h. Internal inspections show the **pedestal walls and structure
   are mostly intact**. The report's own words: the code predictions "seem to
   be overestimated and unrealistic, at least with respect to lateral erosion
   and pedestal wall attack."
2. **Debris morphology on the BWR core plate is unresolved** — whether debris
   accumulates on the plate (which has many openings) or relocates
   independently per assembly. This drives lower-head failure mode.
3. **Control-blade behaviour** — the extent of B₄C exposure to steam, and
   volatile boric acid formation, is unknown.
4. **Upward heat transfer to BWR upper internals** is a major uncertainty; it
   also governs **main-steam-line creep failure**.
5. **Aqueous FP transport** — leakage of FPs in water from the S/C or PCV, and
   leaching from submerged fuel debris, is known to occur and is **not
   generally modelled**. It is a persistent long-term source term.
6. **Revaporisation / revolatilisation** of deposited CsI and Cs₂MoO₄ off
   heating surfaces (MELCOR models it; others model evolving gaseous I₂ and
   CH₃I instead) produces ongoing low-level release between the burst events.
7. **"User effect" is a major source of variability** — the same code in
   different hands gives different answers. Scatter *decreased* from Phase I to
   Phase II as analysts gained BWR experience.
8. **Vessel-breach signatures used in SAMGs are likely problematic**, since
   significant debris can remain in the vessel after initial lower-head
   failure.
9. **Liner melt-through now looks less likely** than the analyses suggest, in
   the timeframe following lower-head failure.
10. Weather-dataset choice **significantly influences both** the forward
    deposition patterns and the backwards-calculated source terms; the report
    recommends harmonising weather data, deposition velocities and rainfall
    between the two activities, and determining uncertainty bands for both.

---

## 6. How to use this for V&V (gh:#1028)

**Rank the targets by what they actually constrain:**

| Tier | Quantity | Why |
|---|---|---|
| **A — measured** | RPV and D/W pressure histories (all three units); CAMS dose rates; RCIC/HPCI/vent/explosion timings; unit 1 PCV pressure over 0–500 h | Observations. A model that misses these is wrong, not merely different |
| **A — observationally reconstructed** | Three-unit integral Cs-137, **13.1 – 14.5 PBq** | Two independent inverse methods agree |
| **B — well-constrained code consensus** | Release from fuel > 80 %; unit 1 lower head 7–17 h; unit 3 lower head 43–58 h; unit 2 H₂ > 800 kg needed to match measured PCV pressure | Wide agreement, and in the H₂ case tied to a measurement |
| **C — wide code spread, weak constraint** | Debris mass in pedestal; concrete erosion depth; per-unit release split; MCCI extent | Spreads of 5–10×; being inside the band proves little |
| **D — known to be wrong** | MCCI persisting to 500 h; extensive lateral pedestal erosion | Contradicted by plant inspection. **Do not calibrate to these** |

**The honest framing for an uncalibrated OUTRAM PARK model** is: report where
it sits relative to the Tier A values, state the Tier B band it falls in or
misses, and do not treat Tier C agreement as validation. Per the workspace rule
on hierarchical surrogates, run it **uncalibrated first and record the
disagreement** — a derived model landing outside the BSAF band is a finding,
not a failure, provided the inputs came from geometry and material data rather
than from the band.

---

## Not extracted

- Chapter 6 (radiological measurements and the GRS/WSPEEDI backwards-calculation
  **methodology**, pp.49–55) — read only far enough to confirm the integral
  values in §4. The monitoring-post dose-rate traces and the hourly release-rate
  reconstructions are figures, not tables, and were not digitised.
- The **full Phase II report** that this summary condenses. Several bands here
  (notably the unit 2 pedestal-mass inconsistency in §2.2) could only be
  resolved there.
- ~~Figures 5.1–5.4 (per-participant Cs-137 release-rate time histories, Bq/hr
  over 20 days). These would be needed to drive a time-dependent atmospheric
  release and are available only as log-scale plots.~~ **DONE — 2026-10-27:**
  digitised, 21 series and 10 403 points, in
  [`nea7525-bsaf-cs137-release-rates/`](nea7525-bsaf-cs137-release-rates).
  Verified by integrating each curve against Table 5.1 (median ratio
  **0.928**; 17 of 21 within a factor of two; ~~9~~ **7** within 5 % —
  **CORRECTED 2026-10-27**, the 9 was the A-grade count, a wider band, and the
  slip was caught by `tests/bsaf_cs137_release_rates.rs`, which recomputes the
  published table from the committed CSVs). Per-series grades and the four
  **D/C** series whose magnitude must not be trusted are in that folder's
  README.
