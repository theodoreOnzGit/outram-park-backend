# Fukushima Daiichi Units 2 and 3: open-literature containment data for `NetworkConfig` / `Params` (gh:#1101)

> Research, education and V&V only. AI draft, unreviewed. Compiled 2026-11-01.

## How to read this file (limits stated first)

- **No primary PDF was opened.** In this session every document host was
  blocked: WebFetch failed with DNS/allow-list errors on japan.kantei.go.jp,
  www.osti.gov, www-pub.iaea.org, fdada.info, world-nuclear.org, www.mdpi.com,
  and `curl` got proxy 403 on nrc.gov, sandia.gov, tepco.co.jp,
  oecd-nea.org, tandfonline.com, simplyinfo.org, web.archive.org and others.
  Only **WebSearch** worked. It returns a search engine's excerpt or
  summary of the named document.
- So every value below is marked **[S]**: it comes from a search excerpt
  attributed to a named public document. The number has **not** been checked
  against the page or table of the primary. That is the same standing as
  `params.rs`'s **[P]/[U]** marks. Treat [S] as [U] until someone reads the
  primary. Page and table numbers are given only where the excerpt showed
  them. Otherwise it says "page not seen".
- **[D]** = derived here by arithmetic from [S] values (the formula is given).
- **[A]** = borrowed from a sister unit or a generic BWR/4 Mark I.
- **Not found** = no open value turned up. Nothing has been invented.
- All accessed 2026-11-01 (via WebSearch).
- For a re-check, the primaries to read first are: **Kantei Attachment IV-1**
  (MAAP conditions per unit); the **NISA "major parameters" tables of March
  2011** (design pressures); **INPO 11-005**; **TEPCO 2016 Unit 2 torus
  survey (NRC ML16267A397)**; the **IRID basic-data page**; and
  **SAND2012-6173** §§ on containment and reactor-building nodalisation. The
  session needs those hosts allowed.

---

## Step 1: what Unit 1 needs, and how Unit 1 sourced it

From `crates/sembawang/src/bwr_mark1/containment_network.rs`
(`NetworkConfig::unit1()`, module docs) and
`crates/sembawang/src/bwr_mark1/params.rs`. The marks are the code's own:
[P] published but unchecked, [U] unverified, [A] assumed. No
`References.md` exists under `crates/sembawang`. The sources are cited in
the `params.rs` header and in the line comments.

### `NetworkConfig` (all [A] for Unit 1)

| field | Unit 1 | how sourced |
|---|---|---|
| `torus_shape` | `Torus` | modelling choice |
| `torus_major_radius_m` | 15.0 | [A] "assumed torus"; minor radius **derived** (3.84 m) so the torus holds `wetwell_gas_m3` + pool |
| `drywell_floor_m` | `None` (= torus top) | [A] |
| `downcomer_entry_m` | 2.0 | [A] above drywell floor |
| `sump_area_m2` / `sump_depth_m` / `sump_water_m3` | 2.0 / 1.5 / 1.0 | [A] numerical (a CV needs a pool) |
| `building_floor_m2` | 650 | [A] |
| `building_openings_m2` | 1.0 | [A] |
| inertial lengths: downcomer / VB / vent / flange / building, m | 8 / 1 / 60 / 0.5 / 1 | [A] |

### `Params` used by the network

| const | Unit 1 | mark and source in code |
|---|---|---|
| `DRYWELL_VOLUME_M3` | 3410 | [U] "KTH slides, simplyinfo" |
| `WETWELL_GAS_M3` | 2620 | [P] Japanese govt report to IAEA, June 2011, attachment 4 |
| `POOL_WATER_M3` | 1750 | [P] same |
| `CONTAINMENT_DESIGN_PA` | 0.43 MPa g + 1.013e5 = 0.5313 MPa abs | [P] NISA via WNA |
| `INITIAL_CONTAINMENT_PA`, `INITIAL_DRYWELL_K`, `INITIAL_POOL_K` | 1.05e5 Pa, 323.15 K, 303.15 K | [A] |
| `VENT_SUBMERGENCE_PA` | 11 800 (~1.2 m) | [A] |
| `DOWNCOMER_AREA_M2` | 20 | [A] |
| `DOWNCOMER_COUNT` | 96 | [A] "Mark I layout of 96" |
| `DISCHARGE_COEFFICIENT` | 0.6 | [A] |
| `VACUUM_BREAKER_PA` / `_M2` | 3400 / 0.5 | [A] |
| `VENT_RUPTURE_DISK_PA` | 0.549 MPa abs | [U] |
| `VENT_AREA_M2` | 0.07 | [A] |
| `FLANGE_LEAK_PA` / `_M2` | 0.75 MPa abs / 5e-4 | [A] |
| `REACTOR_BUILDING_M3` | 26 000 | [A] ("about 38 000 is quoted for Unit 3") |
| `DW_FLOOR_M2` | 200 | [A] |
| `WW_POOL_SURFACE_M2` | 750 | [A] from the assumed 15 m torus |
| `DW_WALL_AREA_M2` / `_H` / `_C` / `_HEIGHT_M` | 4000 / 20 / 5e8 / 20 | [A] "real inventory in SAND2012-6173 (not held)" |
| `DW_GAP_M`, `DW_GAP_EMISSIVITY`, `DW_CONCRETE_M`, `CONCRETE_K`, `RB_WALL_H` | 0.05, 0.8, 1.5, 1.4, 5 | [A] |
| `WW_SHELL_AREA_M2` / `_C` / `_HEIGHT_M` | 1353 / 7.96e7 / 5 | [A] from the assumed torus, 15 mm steel |
| `H2_FLAMMABLE` / `H2_EXPLOSION` | 0.04 / 0.08 | [P] / [A] (unit-independent) |
| RPV: `RPV_RADIUS_M`, `RPV_HEIGHT_M`, `THERMAL_POWER_W`, `ASSEMBLIES` | 2.4 m, 20 m, 1380 MWt, 400 | [U]/[U]/[P]/[P] (vessel model, not the network) |

---

## Step 2: Units 2 and 3

### Source keys

| key | source |
|---|---|
| **KAN** | Government of Japan, *Report of the Japanese Government to the IAEA Ministerial Conference on Nuclear Safety*, June 2011, **Attachment IV-1** "Reactor Core Conditions of Units 1 to 3 of Fukushima Daiichi" (MAAP analysis conditions). https://japan.kantei.go.jp/kan/topics/201106/pdf/attach_04_1.pdf. Page not seen. The search excerpt gives the two MAAP condition sets: (1380 MWt) D/W 3410, pool 1750; (2381 MWt) "D/W open space 4240 m³, S/C open space 3160 m³, suppression pool water volume 2980 m³" |
| **JNST25** | (authors not seen in excerpt), "Fukushima Daiichi Nuclear Power Plant Unit 2 accident analysis considering the thermal stratification and containment leakage", *J. Nucl. Sci. Technol.* (2025), DOI 10.1080/00223131.2025.2570371. Gives Unit 2 D/W 4240, S/C 3160, pool 2980 m³ |
| **SI2 / SI3 / SI1** | SimplyInfo.org, "Reactor 2" / "Reactor 3" / "Reactor 1" status pages, https://simplyinfo.org/fukushima-status/reactors/reactor-2/ (‑3/, ‑1/). This is a secondary compilation by a citizen group. SI3 says its pool figure is "an estimate based on unit 2" |
| **MDPI23** | "CFD Study of Thermal Stratification in a Scaled-Down, Toroidal Suppression Pool of Fukushima Daiichi Type BWR", *Fluids* 8(1):20 (2023), https://www.mdpi.com/2311-5521/8/1/20. Gives the pool at 2980 m³, torus major diameter 33.5 m, minor 8.9 m |
| **TEPCO16** | TEPCO (2016), Unit 2 torus-room / S/C investigation, hosted as NRC ADAMS ML16267A397, https://www.nrc.gov/docs/ML1626/ML16267A397.pdf. Gives "S/C diameter: 8900 mm", "S/C equator OP1,900", Unit 2 R/B 1st floor OP10,200, and post-accident S/C level ≈ OP3,150 to 3,210 |
| **TEPCO17** | TEPCO handout, 15 Feb 2017, https://www.tepco.co.jp/en/nu/fukushima-np/handouts/2017/images/handouts_170215_01-e.pdf. Gives the Unit 2 PCV at ≈33 m high and ≈20 m diameter |
| **TEPCO11** | TEPCO handout 2011-08-10, https://www.tepco.co.jp/en/nu/fukushima-np/images/handouts_110810_04-e.pdf. Gives the S/C of Units 2–5 at ≈3000 m³ of water |
| **TEPCO-OL** | TEPCO plant outline tables, https://www.tepco.co.jp/en/nu/fukushima-np/outline_f1/index-e.html. Gives assemblies 400/548/764, control rods 97/137/185, RPV ID ≈4.8/5.6/6.4 m by plant size |
| **IRID** | IRID, "Basic data of Fukushima Daiichi Nuclear Power Plant (Summary)", https://irid.or.jp/fd/page_id/237/ (data as of 2014-06-25). It has a PCV-size table, U1/U2/U3: row (1) ≈32/34/34 m, (2) ≈10/11/11 m, (3) ≈18/20/20 m, (4) ≈8/9/9 m. **The row labels were not visible in the excerpt.** Inferred to be PCV height, drywell upper-cylinder dia., drywell sphere dia. and S/C tube dia. [U]. It also gives R/B ≈46 m and Unit 3 RPV ≈5.5 m × ≈21 m |
| **NISA** | NISA "major parameters" situation tables, March 2011 (e.g. en20110326-2-3, en20110327-1-3; mirrors at ic.unicamp.br/~stolfi/.../NISA/docs/ and sujb.gov.cz/.../NISA_major_parameters_tab_en20110326-3-3.pdf). They give, for the Unit 2 and 3 columns, "D/W design operating pressure 0.384 MPa g (0.485 MPa abs)" and a maximum of "0.427 MPa g (0.528 MPa abs)" |
| **ICANPS** | Investigation Committee (Japanese Cabinet), material 120224 Siryo 02‑1, https://www.cas.go.jp/jp/seisaku/icanps/eng/120224Siryo02-1Eng.pdf. Gives 1380/2381 MWt and 460/784 MWe, and PCV design pressure ≈0.43 MPa g (U1) and ≈0.38 MPa g (U2–5) |
| **INPO** | INPO 11-005, *Special Report on the Nuclear Accident at the Fukushima Daiichi NPS*, Nov 2011, NRC ML11347A454, https://www.nrc.gov/docs/ml1134/ml11347a454.pdf. Gives the Unit 2 design at 55.1 psig (0.38 MPa g), the rupture disk at "62 psig (427 kPa gauge)", AO-205 (large S/C vent valve), AO-206 (small) and MO-271, and says the Unit 2 R/B blow-out panel opened from the Unit 1 explosion (12 Mar) |
| **AESJ4** | AESJ, *The Fukushima Daiichi Nuclear Accident* (Final Report), Vol. 4, ch. "Measures to Improve Robustness of PCV…", https://www.aesj.net/document/fukushima_vol4/Vol4_32_455_472.pdf. "set working pressure of the rupture disk … 0.528 MPa[abs]" (unit not seen in excerpt); the disk set "according to the maximum working pressure of the PCV" |
| **ANS-A** | ANS Special Committee on Fukushima, Appendix A FAQ (2012), https://www.ans.org/file/3417/Fukushima_Appendix_A.pdf. Gives the Unit 3 rupture disk at "0.549 MPa" and the Unit 1 design pressure as "0.53 MPa (62 psig)" |
| **TEPCO-slide** | TEPCO / Japanese govt slide in the same search set (likely Unit 1): "D/W maximum operating pressure 0.528 MPa abs", rupture disk "0.549 MPa abs", venting pressure 0.954 MPa abs. The unit was not seen |
| **NAS** | US National Academies (2014), *Lessons Learned … Safety of U.S. Nuclear Plants*, timeline tables, https://www.ncbi.nlm.nih.gov/books/NBK253935/. Gives the U1 design at 0.43 MPa and U2/U3 at 0.38 MPa |
| **arX** | "Resilient Design in Nuclear Energy…", arXiv:2303.08868. Gives the design at 0.52 MPa abs (U1) and 0.48 MPa abs (U2, U3) |
| **Spr** | Springer chapter "Accident of Fukushima Daiichi NPP: Sequences, FP Released…" (doi 10.1007/978-4-431-54583-5_3). Uses "design pressure 0.427 MPa" for Unit 3. This is probably the maximum working pressure (NISA) |
| **KTH12** | KTH, EMUG 2012 slides "Fukushima simulation", https://www.psi.ch/sites/default/files/import/emug/Emug2012EN/EMUG2012_Fukushima_simulation_KTH.pdf. Gives the Unit 3 reactor building free volume at **38 000 m³**, with an asterisk (meaning not seen). It also gives the Unit 3 RPV at ≈21 m high and 5.6 m ID, with 548 FA and 137 rods |
| **NRC-HRTD** | NRC HRTD GE BWR/4 ATM ch. 7.1 "Fukushima Dai-ichi Event" (held in repo: `crates/kovan-literature/reactor-literature/theodore-open-corpus/nrc/nrc-hrtd-ge-bwr4-atm-ch7.1-fukushima.pdf`, read locally). It gives a "typical BWR-4 reactor building volume is about 1.5 million cubic feet" (≈42 500 m³), generic, and says the hardened vents were installed 1999–2001 |
| **MkI-sub** | NRC-referenced Mark I Owners Group study on reduced downcomer submergence (search excerpt; NRC ML19263C762). Gives Mark I minimum submergence from "3.00 feet" upward, and finds 3 ft acceptable. Generic |
| **NI-vent** | NRC information notice (search excerpt): "8 to 10 vent pipes … in most Mark I containments". Generic |
| **J3-VB** | "Analysis of Fukushima-Daiichi NPP Unit 3 pressure data…", *Nucl. Eng. Des.* (2021), S0029549321003782. A D/W–S/C difference of ≈0.035 MPa "corresponds roughly to a water head between the level of S/C vacuum break valve and the edge of S/C down-comer" (a geometric head, **not** the breaker set point) |

### 2.1 Free volumes and pool

- **Drywell free volume**
  - U2 = U3 = **4240 m³** [S] (KAN, JNST25, SI3).
  - KAN's table labels the column by power (2381 MWt), not by unit. JNST25
    names Unit 2 explicitly. SI3 names Unit 3.
  - Differs from U1 (3410).
- **Wetwell (S/C) gas space**
  - U2 = U3 = **3160 m³** [S] (KAN, JNST25, SI3).
  - Differs from U1 (2620).
- **Suppression pool water**
  - U2 = U3 = **2980 m³** [S] (KAN, JNST25, MDPI23, SI2).
  - TEPCO11 and WNA round it to ≈3000 m³ for Units 2–5.
  - SI2 labels it "2980 t". That is a unit slip and does not change the value
    materially.
  - SI3 says Unit 3's figure is "an estimate based on unit 2". So for U3 it
    is effectively **[A] from U2**, although KAN applies it to the
    2381 MWt class.
  - Differs from U1 (1750).

### 2.2 Torus geometry

- **Major diameter**
  - U2 **33.5 m** (R = 16.75 m) [S] (SI2, MDPI23).
  - U3 **33.5 m** [S] (SI3).
  - U1 by the same compiler: 29.6 m (R = 14.8 m) [S] (SI1). Code: R = 15 [A].
- **Minor (tube) inner diameter**
  - U2 **8.9 m** (r = 4.45 m) [S] (SI2, MDPI23, TEPCO16 "S/C diameter
    8900 mm").
  - U3 **8.9 m** [S] (SI3).
  - IRID row (4) ≈9 m for U2/U3 and ≈8 m for U1 [U-label].
  - U1: 8.08 m (r = 4.04 m) [S] (SI1).
  - **Conflict with the U1 code:** the code derives r = 3.84 m by fitting
    gas + pool into a 15 m torus. SI1's geometric 4.04 m gives a torus of
    4768 m³ against 4370 m³ of gas + pool (ratio 0.916). The remaining
    ≈8 % is presumably the vent header, downcomers, ring girders and other
    internals. The same check on U2/U3 gives 6547 m³ against 6140 m³ (ratio
    0.938) [D], so the two units are consistent.
- **Torus elevation, U2:** S/C equator at **OP 1,900 mm** [S] (TEPCO16).
  - So the torus bottom is at ≈OP −2,550 mm [D] (equator − 4.45 m).
  - R/B 1st floor at OP 10,200 mm [S].
  - U3: not found. U1: not found in this session.
- **Normal pool level / pool surface area:** no published normal level found.
  - [D] from the volumes on the geometric torus: pool fraction
    2980/(2980+3160) = 0.485 of the free space, so a level of ≈4.35 m above
    the tube bottom (≈0.1 m below the equator).
  - Free surface ≈ 2πR × chord ≈ **936 m²** [D] (U2/U3).
  - The same arithmetic for U1 gives 3.41 m and 742 m² (code: 750 m²
    prismatic, 711 m² from its torus).
  - The post-accident U2 S/C level of OP 3,150 to 3,210 (TEPCO16) is a
    flooded-torus-room state, **not** the normal level.
- **Torus shell area (geometric):** U2/U3 4π²Rr ≈ 2943 m² [D]. U1 ≈2360 m²
  [D] (code assumed 2458).

### 2.3 Vents, downcomers, vacuum breakers

- **Main vent pipes (drywell to header)**
  - U2/U3: **8**, ID **2.06 m** [S] (SI2, SI3).
  - U1: 8, ID 1.75 m, header ID 1.25 m [S] (SI1).
  - Generic Mark I: 8–10 vent pipes (NI-vent).
  - [D] flow area: U2/U3 8·π/4·2.06² = **26.7 m²**. U1 = 19.2 m².
  - **Note:** the U1 code's `DOWNCOMER_AREA_M2` = 20 [A] is within 4 % of
    SI1's vent-pipe area. That is a coincidence, not a citation.
- **Downcomer count (inside torus)**
  - U2 **96**, U3 **96** [S] (SI2, SI3).
  - **U1 80** [S] (SI1). The code has 96 [A], which **conflicts** with SI1.
- **Downcomer individual diameter: not found** for any unit.
  - A generic ≈24 in (0.61 m) was searched for and **not confirmed**.
  - If assumed [A], 96 × 0.61 m gives ≈27–28 m², close to the vent-pipe
    area, as a Mark I design would be.
- **Downcomer submergence: not found** for Units 1–3.
  - Generic Mark I: minimum 3.00 ft (0.91 m) upward, 3 ft acceptable
    (MkI-sub) [A].
  - The U1 code uses 1.2 m [A].
  - J3-VB's ≈0.035 MPa (≈3.6 m of water) is the head from the breaker level
    to the downcomer edge in U3 [S]. It is a geometric constraint and could
    help place the breakers relative to the downcomer exit.
- **Vacuum breakers (S/C to D/W): count, area and set point not found** for
  any unit.
  - TEPCO16 confirms Unit 2 has vacuum-breaker lines with bellows (expansion
    joints). One leaked in 2016.
  - U1 code: 3400 Pa / 0.5 m² [A].
  - The generic 0.5 psid (3.4 kPa) set point was searched for and **not
    confirmed** by an open excerpt.

### 2.4 Design pressure, wetwell vent, rupture disk

- **PCV design pressure**
  - U2 = U3 = **0.384 MPa g = 0.485 MPa abs** [S] (NISA, "D/W design
    operating pressure").
  - Corroborated as ≈0.38 MPa g by ICANPS, INPO (U2 55.1 psig), NAS and
    arX (0.48 MPa abs).
  - **Differs from U1** (0.43 MPa g).
  - **Conflict:** Spr uses 0.427 MPa for U3. That matches NISA's *maximum*
    operating pressure, 0.427 MPa g (0.528 MPa abs), not the design value.
  - Also for U1: ANS-A gives 0.53 MPa (62 psig), and INPO gives 62.1 psig
    (0.428 MPa g). ANS-A's MPa figure is probably absolute. Both are
    consistent with the code's 0.43 MPa g.
- **Maximum operating pressure:** U2/U3 **0.427 MPa g / 0.528 MPa abs** [S]
  (NISA).
- **Rupture-disk set pressure.** Sources disagree:
  - **0.427 MPa g (62 psig) = 0.528 MPa abs** [S]. INPO: "62 psig (427 kPa
    gauge) pressure necessary to [open the] rupture disk". The search
    excerpts attach this to the **Unit 2** S/C vent, and one excerpt to the
    Unit 3 context of 13 March. AESJ4 gives 0.528 MPa abs, with the disk set
    "according to the maximum working pressure of the PCV". That matches
    NISA's U2/U3 maximum of 0.427 MPa g.
  - **0.549 MPa abs** [S]. ANS-A gives this for the **Unit 3** hard-piped
    vent. TEPCO-slide gives 0.549 MPa abs alongside a 0.528 MPa abs
    "D/W maximum operating pressure", which looks like **Unit 1** and
    matches the code's U1 value of 0.549 MPa abs [U].
  - **Best reading, unconfirmed:**
    - U1 0.549 MPa abs.
    - U2 0.528 MPa abs (0.427 MPa g) [S, INPO + AESJ4].
    - U3 **unresolved**: 0.528 MPa abs (by the design rule AESJ4 states and
      NISA's U3 maximum) or 0.549 MPa abs (ANS-A).
  - Read IAEA TV1 and the TEPCO June 2012 Accident Analysis Report to settle
    it.
- **Vent line flow area / pipe size: not found** for any unit.
  - The valve tags are known: U2 AO-205 (large S/C vent valve), AO-206
    (small, parallel) and MO-271 (motor-operated).
  - A nominal size (300A, 350A, 250A) was searched for and not confirmed.
  - The code's U1 0.07 m² stays [A].
- **Unit 2 vent behaviour (for `Mode::Historical`, not a geometry datum):**
  the line was lined up to the disk, but S/C pressure stayed ≈300–400 kPa
  abs, below the disk, so U2 never vented through it [S] (TEPCO interim
  report, IAEA TV1 excerpts).
  - U3: the disk burst at ≈09:20 on 13 March, with the D/W drop confirmed at
    09:24 [S] (IAEA TV1, Spr).
- **Drywell vent line (U2): not found** as a separate area or disk value.
- **Head-flange leak: not found** (U1 [A] too).

### 2.5 Reactor building

- **Free volume**
  - U3: **38 000 m³** [S] (KTH12, marked with an asterisk in the table,
    meaning not seen).
  - U2: not found. [A] candidates: U3's 38 000 (same plant size), or the
    generic BWR-4 "≈1.5 million ft³" ≈ 42 500 m³ (NRC-HRTD, read locally).
  - U1 code: 26 000 [A].
  - A "refueling bay 21 000 m³" (NAS) and "≈6000 m³" (a MELCOR model's
    single volume, OSTI 1333851) appeared in excerpts without a clear
    unit or space. Not used.
- **Outer dimensions:** U2 and U3 ≈46 m wide (N–S) × 46 m grade to roof,
  walls 0.5–1.5 m [S] (SI2, SI3, IRID).
- **Unit 2 blow-out panel**
  - It **opened from the Unit 1 explosion's pressure wave (12 Mar 2011)**
    [S] (INPO, Fauske citing Rempe et al.).
  - It was closed by TEPCO on 2013-03-11 (TEPCO photo library).
  - **Size: not found.** "Large, rectangular" is all that was found (CRS
    R41694). A 6 m × 5 m figure was searched for and not confirmed.
- **Unit 3:** the building exploded at 11:01 on 14 Mar. After that the
  openings are effectively unbounded. This is sequence data, not a
  configuration value.

### 2.6 RPV and core (for `Params`, not the network)

- **Thermal power:** U2 = U3 = **2381 MWt** (784 MWe) [S] (ICANPS).
- **Fuel assemblies:** 548. **Control rods:** 137 [S] (TEPCO-OL, KTH12).
  - U3 carried 32 MOX assemblies. This is from memory, **not** confirmed in
    this session.
- **RPV inner diameter:** ≈**5.6 m** [S] (TEPCO-OL, KTH12). SI2/SI3 give
  5.57 m. IRID gives 5.5 m for U3.
- **RPV height:** 22.0 m (SI2, SI3), against ≈21 m (KTH12, IRID). This is a
  **conflict**.
- **RPV free volume and water inventory:** not found (KAN would hold the MAAP
  inventory; not seen).
- **PCV overall:** U2 ≈33 m high, ≈20 m dia. (TEPCO17), against ≈34 m
  (IRID row 1). Small **conflict**.

### 2.7 Structure inventories (drywell steel and concrete, torus shell mass)

- **Not found.**
  - SAND2012-6173 has the heat-structure and nodalisation sections, but OSTI
    was blocked.
  - SAND's Unit 3 model was built from the SOARCA Peach Bottom Mark I
    template with surrogate data (OSTI 1249163 slides). So even SAND's U3
    structure inventory is partly **[A] from Peach Bottom**, a much larger
    BWR/4.
- **Drywell shell:** "30 mm steel backed by reinforced concrete" (WNA,
  generic for units 1–5). U1 is quoted elsewhere as 16 mm (INVAP). Both [S],
  page not seen.

---

## Summary table

U1 values are as in code. U2/U3 values are [S] unless marked. Pa abs unless
"g".

| parameter | Unit 1 (code) | Unit 2 | Unit 3 | source keys |
|---|---|---|---|---|
| drywell free volume, m³ | 3410 [U] | 4240 | 4240 | KAN, JNST25, SI3 |
| wetwell gas space, m³ | 2620 [P] | 3160 | 3160 | KAN, JNST25, SI3 |
| pool water, m³ | 1750 [P] | 2980 | 2980 (SI3: estimated from U2) | KAN, JNST25, MDPI23, TEPCO11 |
| torus major radius, m | 15.0 [A] (SI1: 14.8) | 16.75 | 16.75 | SI2, SI3, MDPI23 |
| torus minor radius, m | 3.84 derived [A] (SI1: 4.04) | 4.45 | 4.45 | SI2, SI3, MDPI23, TEPCO16, IRID |
| torus equator elevation | — | OP 1,900 mm | not found | TEPCO16 |
| pool surface at normal level, m² | 750 [A] / 711 torus | ≈936 [D] | ≈936 [D] | derived |
| main vent pipes (count × ID) | not in code (SI1: 8 × 1.75 m) | 8 × 2.06 m (26.7 m² [D]) | 8 × 2.06 m | SI1–3 |
| downcomer count | 96 [A] (**SI1: 80**) | 96 | 96 | SI1–3 |
| downcomer total area, m² | 20 [A] | not found ([A] ≈27 if 0.61 m each) | not found | — |
| downcomer submergence | 11 800 Pa (~1.2 m) [A] | not found ([A] generic ≥0.91 m) | not found | MkI-sub |
| vacuum breaker set point / area | 3400 Pa / 0.5 m² [A] | not found (lines with bellows exist) | not found | TEPCO16 |
| design pressure | 0.43 MPa g [P] | 0.384 MPa g (0.485 abs) | 0.384 MPa g (Spr: 0.427, conflict) | NISA, ICANPS, INPO, NAS, arX, Spr |
| max. operating pressure | — | 0.427 MPa g (0.528 abs) | 0.427 MPa g (0.528 abs) | NISA |
| rupture disk | 0.549 MPa abs [U] | 0.528 abs (427 kPa g) | **conflict**: 0.528 vs 0.549 abs | INPO, AESJ4, ANS-A, TEPCO-slide |
| vent flow area, m² | 0.07 [A] | not found | not found | — |
| head-flange leak | 0.75 MPa / 5e-4 m² [A] | not found | not found | — |
| reactor building free volume, m³ | 26 000 [A] | not found ([A] 38 000 from U3, or 42 500 generic BWR-4) | 38 000 (asterisked) | KTH12, NRC-HRTD |
| R/B outer size | — | 46 × 46 m (width × height) | 46 × 46 m | SI2, SI3, IRID |
| building openings | 1 m² [A] | blow-out panel open from 12 Mar; **size not found** | building destroyed 14 Mar | INPO, CRS |
| drywell structures (area, mass) | 4000 m², 5e8 J/K [A] | not found | not found (SAND U3 is Peach Bottom surrogate) | — |
| torus shell area (geometric) | 1353 above pool [A] (of 2458) | ≈2943 total [D] | ≈2943 total [D] | derived |
| thermal power | 1380 MWt [P] | 2381 MWt | 2381 MWt | ICANPS |
| assemblies / control rods | 400 / 97 | 548 / 137 | 548 / 137 | TEPCO-OL, KTH12 |
| RPV ID / height | 4.8 m / 20 m [U] | ≈5.6 m / 22 vs 21 m (conflict) | ≈5.6 (IRID 5.5) / 21–22 m | TEPCO-OL, SI2, SI3, KTH12, IRID |
| PCV height | — | ≈33 m (TEPCO17) vs ≈34 m (IRID) | ≈34 m | TEPCO17, IRID |

---

## Step 3: in the code (gh:#1101, 2026-11-01)

`Params::unit2()` / `Params::unit3()` (`src/bwr_mark1/params.rs`) and
`NetworkConfig::unit2()` / `NetworkConfig::unit3()`
(`src/bwr_mark1/containment_network.rs`) carry the values above. Each
field's mark is in a comment beside it. The marks are those of this file:
[S] is a search excerpt and the primary was not opened, so treat it as
[U]; [D] is derived and the derivation is shown; [A] is assumed. A field
not found is borrowed from Unit 1 and marked [A]. Tests:
`tests/units_2_and_3_network.rs` (geometry from the assembled network, and
a 300 s relief run that conserves) and
`params::tests::units_2_and_3_derive_from_their_sources`.

~~**No comparison with the accident.** The crate has no RCIC or HPCI model
and no Unit 2 or Unit 3 sequence. `Mode::Historical` is Unit 1's. A
Unit 2 or 3 network can be driven only by a snapshot the caller builds.~~
**CORRECTED 2026-11-02 (gh:#1109):** `turbine_pump` (RCIC, HPCI) and
`units23` (each unit's sequence from USNRC HRTD ch. 7.1 Table 7.1-2) now
exist, and `Plant::for_unit` runs them under `Mode::Historical`; the
comparison is [`units23_sequence.md`](units23_sequence.md).

### Field by field

`Params` fields not listed are Unit 1's defaults, borrowed [A].

| field | Unit 2 | Unit 3 | mark | source or derivation |
|---|---|---|---|---|
| `Params::thermal_power_w` | 2381 MWt | same | [S] | ICANPS |
| `assemblies`, `control_rods` | 548, 137 | same | [S] | TEPCO-OL, KTH12 |
| `rpv_radius_m` | 2.8 m | same | [S] | 5.6 m ID, TEPCO-OL, KTH12 |
| `rpv_height_m` | 22 m | 21 m | [S] | U2: SI2 (its only source). U3: IRID and KTH12, two sources for Unit 3 itself; SI3's 22 m recorded, not used |
| `cs_`, `i_`, `te_inventory_kg` | Unit 1's × 1.7254 | same | [D] | power ratio 2381/1380 (ICANPS) × Unit 1's [P]/[A]; assumes the same irradiation history [A]. Unit 3's MOX is not represented |
| decay heat | from `thermal_power_w` | same | [D] | `decay.rs` scales with the rated power. Fission shares are MELCOR's defaults [A], as for Unit 1 |
| `drywell_volume_m3` | 4240 m³ | same | [S] | KAN, JNST25, SI3 |
| `wetwell_gas_m3` | 3160 m³ | same | [S] | KAN, JNST25, SI3 |
| `pool_water_m3` | 2980 m³ | same | [S] | KAN, JNST25, MDPI23. U3: SI3 calls it "an estimate based on unit 2", so it is effectively [A] from Unit 2 |
| `containment_design_pa` | 0.384 MPa g | same | [S] | NISA |
| `vent_rupture_disk_pa` | 0.528 MPa abs | 0.528 MPa abs | [S] | U2: INPO, AESJ4. U3: **conflict**. 0.528 was chosen because AESJ4's rule sets the disk at the PCV maximum working pressure, which NISA gives as 0.427 MPa g = 0.528 MPa abs. ANS-A's 0.549 is kept as `UNIT3_RUPTURE_DISK_ANS_PA`. It equals Unit 1's code value and may be carried over from Unit 1. Not averaged |
| `downcomer_count` | 96 | 96 | [S] | SI2, SI3 |
| `downcomer_area_m2` | 27.71 m² | same | [D] | Unit 1's 20 m² [A] × the main-vent area ratio (2.06/1.75)² (SI1, SI2). Each downcomer is then 0.606 m across. That 0.606 m is close to the unconfirmed generic 0.61 m, but the generic value was not used |
| `dw_floor_m2`, `dw_wall_area_m2`, `dw_wall_c` | Unit 1's × 1.1563 | same | [D] | Unit 1's [A] × (4240/3410)^(2/3) (KAN), assuming geometric similarity and the same shell thickness [A] |
| `ww_pool_surface_m2` | 906.3 m² | same | [D] | the network's volume-fit torus at the initial level. §2.2's 936 m² is the geometric tube (r = 4.45 m) |
| `ww_shell_area_m2`, `ww_shell_c` | 1492.9 m², 8.789e7 J/K | same | [D] | 4π²Rr = 2942.6 m² (R 16.75 m, r 4.45 m [S]) × 0.50733 above the pool's level. Unit 1's 15 mm steel [A] |
| `reactor_building_m3` | 38 000 m³ | 38 000 m³ | U2 [A], U3 [S] | U3: KTH12. U2: **borrowed from Unit 3**, not from Unit 1 (same plant size; Unit 1's 26 000 is itself [A]) |
| vent area, vacuum breakers, submergence, flange, initial state | Unit 1's | same | [A] | borrowed from Unit 1, not found |
| `NetworkConfig::torus_major_radius_m` | 16.75 m | same | [S] | SI2, SI3, MDPI23. The tube radius stays derived from the volumes (4.309 m). The free volume is 0.9375 of the sourced geometric torus |
| `building_floor_m2` | 646.8 m² | same | [D] | 38 000 m³ / 58.75 m. The 58.75 m runs from the torus bottom (OP −2.55 m, TEPCO16) to the roof 46 m above grade (SI2, IRID), with grade taken as the R/B first floor, OP 10.2 m (TEPCO16) [A]. Unit 3 uses Unit 2's elevations [A] |
| `drywell_floor_m`, `downcomer_entry_m`, sump pits, `pool_layers`, `quencher_elevation_m`, `building_openings_m2`, inertial lengths | Unit 1's | same | [A] | borrowed from Unit 1 |

The following Unit 1 values are scale-dependent but were **not** scaled,
because no sourced ratio fits them: relief-valve count and throat, main
steam lines, dome internals, vessel water levels. The isolation-condenser
fields do not apply to Units 2 and 3.

### What a real Unit 2 or Unit 3 run still needs

- ~~**A sequence.** RCIC and HPCI models and their timing, the relief-valve
  openings, the vent orders and the fire-engine injection, each from an
  open source. None of this exists in the crate, and no timing was
  gathered for this file.~~ **Done 2026-11-02 (gh:#1109)**, see
  [`units23_sequence.md`](units23_sequence.md); still missing there: when
  Unit 2's RCIC suction moved to the torus, and torus suction itself.
- **The data marked "not found" above.** Downcomer diameter and submergence,
  vacuum breakers, vent-line area, Unit 2's building volume and blow-out
  panel, structure inventories, the head flange, the drywell's elevation,
  and Unit 3's torus elevation.
- **The conflicts settled.** Unit 3's rupture disk and the RPV height, from
  IAEA TV1 and TEPCO's June 2012 report.
- **Every [S] confirmed** against its primary (see "How to read this
  file").
