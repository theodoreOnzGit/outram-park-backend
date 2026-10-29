# MELCOR 2.2 manuals: a digest for the Fukushima Unit 1 model

This record lists the models, equations and **default values** in the
public description of MELCOR 2.2 that bear on OUTRAM PARK's Fukushima Daiichi
Unit 1 model (`sembawang::bwr_mark1`, gh:#1028). It is organised by the items
of `docs/fukushima-melcor-gap-plan.md`, and each entry says how the model
compares.

**Clean room.** These are the manuals only. No MELCOR source is held or used
(MELCOR is distributed under an NDA). Everything here is written in our own
words, with the equations restated, and each entry is cited to volume,
section, equation or table, and page label.

---

## Provenance

| Field | Value |
|---|---|
| Sources | **V2**: Sandia National Laboratories (2023), *MELCOR Computer Code Manuals, Vol. 2: Reference Manual, Version 2.2 r2023.0*, SAND2023-10994 O, 1041 pp (NRC ADAMS ML23116A100). **V1**: Sandia National Laboratories (2017), *MELCOR Computer Code Manuals, Vol. 1: Primer and Users' Guide, Version 2.2.9541*, SAND2017-0455 O, 1170 pp (ML17040A429) |
| Copies held | The maintainer's private repository, `propreitrary-literature-theodore`, under `reports/` (added 2026-10-29, commit `98298ab`). Catalogue keys `sandia2023melcorvol2` and `sandia2017melcorvol1` |
| Access tier | **Proprietary.** The front matter has no public-release or unlimited-distribution marking and no copyright or licence statement. These are Sandia contractor reports, not NUREG publications, and ADAMS hosting is no redistribution basis (`CATALOGUE.md`). The PDFs and their extracted text stay private. This record holds facts, equations and numbers, which are not copyrightable, with citation |
| Processing | Text was extracted with `pdftotext -layout` to a session scratchpad, outside every repository. Four AI agents each read one package group on 2026-10-29, and the Urbanic-Heidrick and heat-of-reaction entries were read again by the session itself. Page labels are the manuals' own (for example `COR-RM-97`). Entries marked **[inferred]** are the reader's arithmetic or reasoning, not the manual's text. **Not reviewed by a human** |
| Editions | V2 (2023) and V1 (2017) are different builds. Where they disagree, both values are recorded and the volume is named (§ "Contradictions inside the manuals") |

---

## F3. Lower head, core support, lower-plenum debris (COR package)

**Nodalisation** (V2 COR §1.1.2 and §6.1, COR-RM-17 to 21 and 166 to 170)
- The head is split into user segments, each with NLH through-wall nodes.
  The default layer material is stainless steel.
- The default melting points are 1700 K for stainless steel and 1810.9 K
  for carbon steel (MP-RM-49).
- The conduction solve is semi-implicit and 2-D: an implicit lateral solve
  feeds an implicit 1-D through-wall solve (Eqs 6-6 and 6-7).
- For a BWR the model covers everything below the baffle plate.

**Outer surface** (Eq 6-8)
- Dry cavity: 10 W/m²K (C1246(1)).
- Flooded cavity: downward-facing saturated boiling, with CHF and minimum
  film boiling depending on inclination (Eqs 6-9 to 6-14, C1245).

**Debris to head** (V2 Eqs 6-1, 6-2, 6-5; V1 COR_LHF, COR-UG-143)
- `q = h A (T_debris - T_inner wall)`, with a separate path to the
  penetrations.
- Defaults: debris-to-penetration, debris-to-head, oxide-pool-to-head and
  metal-pool-to-head are each 1000 W/m²K. V1 calls these order-of-magnitude
  values meant for sensitivity studies.
- The pools can use instead `Nu = A Ra^n Pr^m` (V2 Table 2-4, C1280).

**Failure: any one of these** (V2 §6.3, COR-RM-176 to 180)
1. A penetration or the innermost head node reaches **TPFAIL = 1273.15 K**
   (V1 COR_LHF field 5).
2. A user control function.
3. Overpressure during falling-debris quench: PPFAIL = 2.0e7 Pa.
4. **Larson-Miller creep** (SA533B1, from NUREG/CR-5582):
   - `t_R = 10^(P_LM/T - 7.042)` s, with
     `P_LM = 4.812e4 - 4.725e3 log10(sigma_e [Pa])` (Eqs 6-23, 6-24;
     C1601(1..3)).
   - Plastic strain grows by `0.18 dt/t_R`, and the head fails at **0.18**
     (C1601(4)).
   - 0-D stress: `sigma_e = (dP + rho_d g dz_d) R_i^2/(R_o^2 - R_i^2)`
     (Eq 6-26).
   - Creep is skipped while dP is below 1000 Pa (C1600(3)).

**What failure does** (V2 §6.3, §6.4, COR-RM-180 to 183)
- Default opening: 0.1 m diameter (7.9e-3 m²).
- The opening grows by ablation, with the Pilch-Tarbell coefficient
  (Eqs 6-36 to 6-38).
- Ejection velocity: `C_d (2 dP/rho + 2 g dz)^(1/2)`, with `C_d` = 1.0
  (Eq 6-34).
- Ejection is gated: it needs at least 5000 kg molten or a melt fraction of
  0.1 (C1610(1..2)).

**Core support, BWR** (V2 §5.2, §5.3, COR-RM-159 to 166; V1 COR_SS)
- Default: temperature failure, **TSFAIL = 1273.15 K**.
- Optional stress models: PLATEB (the core plate; Eq 5-13, `K_B` = 0.0833)
  and COLUMN (the control-rod guide tubes; Eq 5-15).
- Their failure modes: yield (C1606), Euler buckling (C1605), and creep with
  `t_R = 10^(P_LM/T - 16.44)` and `P_LM = 81000 - 7500 log10 sigma_e`
  (C1604).

**Lower-plenum quench and dryout** (V2 §2.3.8, COR-RM-67 to 70)
- Falling debris exchanges heat with the water at 100 W/m²K over the
  particle area 6V/D (V1 default HDBH2O). This is a coefficient, not an
  instant quench.
- Once the debris reaches the head the coefficient decays (Eq 2-113).
- Heat removal is then capped by **Lipinski 0-D dryout** (Eqs 2-114, 2-115;
  coefficient 0.756, capillary head 0.089 m, porosity at least 0.15; C1244).

**Against `bwr_mark1`**

| Model input | MELCOR | Comparable? |
|---|---|---|
| `lower_head_failure_k` 1500 K \[A\], on the **debris** temperature | TPFAIL 1273.15 K on a **wall or penetration node**, plus creep | **No.** Different quantity. MELCOR's node lags the debris through a 1000 W/m²K coefficient and the wall's heat capacity |
| `breach_m2` 0.01 m² | 7.9e-3 m² default, grows by ablation | Yes. We have no ejection gate |
| `debris_quench_s` 120 s, removal unlimited while water is present | 100 W/m²K, then the Lipinski cap | **No.** Ours has no dryout limit |

---

## F4. Core degradation (COR package)

**Oxide-shell holdup** (V2 §3.1.3, COR-RM-134; V1 C1131)
- Molten Zr is held in the cladding while the ZrO2 is thicker than 1e-5 m
  and the temperature is below **2400 K** (C1131(2)).
- For a BWR canister the breach temperature is 2100 K (C1131(6)).
- Steel: 1e-3 m oxide and 1700 K.
- Default melting points: Zircaloy **2098 K**, ZrO2 2990 K, UO2 3113 K
  (MP-RM-49).

**Candling** (V2 §3.1.1, COR-RM-125 to 128; V1 COR_CHT)
- A film flows down and refreezes: `q = h_m P_w dz (T_m - T_s)` (Eq 3-1).
- The mass refrozen is `(Q - Q_sensible)/H_f` (Eq 3-8).
- Refreezing coefficients, "2.0" defaults: Zr, ZrO2 and UO2 7500 W/m²K;
  steel and control poison 2500 W/m²K. The "1.86" scheme used 1000 for all.
- There is no momentum equation.
- Blockages are tracked on 10 sub-nodes per cell. Melt above a blockage
  becomes molten pool (§3.1.2).

**Rod collapse** (V2 §3.2.1 and §3.2.2, COR-RM-141 to 143; V1 C1132, COR_ROD)
- Non-fuel components collapse at 1e-4 m of remaining metal.
- Oxidised fuel stands until **2500 K** (C1132(1); 2800 K in 1.8.4, lowered
  on Phebus evidence) and always collapses at 3100 K.
- Time-at-temperature damage: lifetime `1/L(T) = A exp(B T)`, with
  `A` = 2.16e-11 s⁻¹ and `B` = 7e-3 K⁻¹ (Eq 3-29; 50th percentile, VERCORS
  plus the SOARCA elicitation). Failure when `sum dt/L = 1`. On by default.
  - **[inferred]** L is about 19 000 s at 2100 K and 1 160 s at 2500 K.
- Collapsed material stays in the core as particulate debris, held up by
  the core support until that fails.

**Oxidation** (V2 §2.5, COR-RM-90 to 99; Table 2-6; V1 §1001, §1004)
- Parabolic, Urbanic-Heidrick, in steam:
  - **A = 29.6, B = 16820 K** for 1100–1853 K;
  - **A = 87.9, B = 16610 K** above 1873 K;
  - linear interpolation in T between the branches (Eq 2-174).
- In oxygen: 50.4 and 14630 K.
- Cutoff: no oxidation below **TOXMIN = 1100 K** (C1004(1)).
- Gaseous-diffusion limit: Eq 2-168 (C1003).
- Relocated material oxidises too: particulate debris over 6V/D (Eq 3-28),
  conglomerate debris, and unquenched surfaces below the pool.
- Optional kinetics (Table 2-7):
  - Cathcart-Pawel / Urbanic-Heidrick: 294.2, 20111 K / 87.9, 16610 K;
  - Leistikow-Schanz / Prater-Courtright;
  - Sokolov;
  - Grosse.

**Heat of reaction** (V2 §2.5.2, COR-RM-95/96, Table 2-5, Eqs 2-166/167)
- Zircaloy in steam: **5,797 kJ/kg Zr**; in air: 12,065 kJ/kg.
- This is a value at MELCOR's own equation-of-state reference points. It is
  corrected to the local temperature through the enthalpies of reactants
  and products (Eq 2-166).
- So it is **not** directly comparable with a 298.15 K standard enthalpy.

**Against `bwr_mark1`**

| Model input | MELCOR | Comparable? |
|---|---|---|
| `uh_*` (six constants) | Table 2-6, identical | **Yes, checked 2026-10-29**; tags \[U\] to \[P\] |
| `oxidation_min_k` 1000 K \[A\] | TOXMIN 1100 K | Yes. **[inferred]** the 1000–1100 K band adds under 1 % of the H2 |
| `zr_melt_k` 2100 K: the whole node slumps to the lower plenum | 2098 K melting, but cladding holds to 2400 K, fuel stands to 2500 or 3100 K or time-at-temperature, and debris is held in-core by the support | **Partly.** Same melting point; different consequence |
| `oxidation_heat_j_per_kg_zr` 6.763 MJ/kg (298 K standard, OFFBEAT) | 5,797 kJ/kg at MELCOR's references, temperature-corrected | **No** (different reference). Ours is now booked by Hess's law at the node, gh:#1046 |
| No gaseous-diffusion limit; no oxidation of relocated Zr | Both present | Gaps |

---

## F5. RPV two-phase mixture level (CVH package)

- Bubble rise applies only in **nonequilibrium** volumes. An equilibrium
  volume has no swell (V2 CVH/FL §5.1, CVH/FL-RM-41).
- The pool model has no drift flux. Bubbles rise at a fixed velocity,
  **0.3 m/s** (C4407(1)), with a vapour flux that grows linearly with height.
  The manual says other assumptions change the answer by about a factor of 2
  (§5.1.3, CVH/FL-RM-47).
- Mean void: `alpha = J_max Z_P/(2 v0 V_P)`, where `V_P` is the swollen
  volume (Eq 5-26). The fraction of bubbles retained over a step is
  `f = 1/(1 + 2 v0 dt/Z_P)`.
- **Maximum pool void: 0.40** (C4407(11)), roughly the bubbly-flow limit.
  Extra vapour goes to the atmosphere.
- Every package, COR included, sees the **swollen** level.
- COR splits each component into quenched and unquenched parts, so coverage
  is fractional within a cell (COR-RM-41 to 43).
- The quench front is tracked separately (Dua-Tien, Eq 2-60). It never sits
  above the two-phase surface.
- A component quenches at once below 40 K superheat (C1260). Quench vapour
  goes straight to the atmosphere and does not add to swell.

**Against `bwr_mark1`:** coverage is decided by the **collapsed** level, with
a yes/no per node at its midpoint.
- **[inferred]** At 7 MPa and about 15 MW the void is about 5 %, a few tens
  of cm of swell.
- **[inferred]** At low pressure the void reaches the 0.4 cap.

---

## F6. Heat structures, in-core radiation, creep (HS, COR, CF packages)

**HS package** (V2 HS §1, HS-RM-5 to 7)
- 1-D conduction in rectangular, cylindrical, spherical or hemispherical
  geometry, with user nodes and an MP material per interval.
- Boundary conditions: adiabatic, convective with computed or specified h,
  specified temperature, or specified flux.
- A convective face needs: its CV, elevation, area, characteristic and axial
  lengths, and a critical pool fraction.

**HS radiation** (§2.6.2, HS-RM-45 to 47)
- To the gas: an equivalent-band model, or a gray gas with
  `q = sigma [1/eps_g + 1/eps_w - 1]^-1 (T_g^4 - T_w^4)` (Eq 2-95). Only
  H2O and CO2 absorb.
- Between pairs of structures: the gray-body series resistance (Eq 2-96).
- Only the area above the swollen level radiates.

**COR radiation** (V2 COR §2.1, COR-RM-26 to 40)
- Radiosity between gray surfaces across a gray steam medium (Eqs 2-1 to
  2-6).
- Zircaloy emissivity is interpolated against oxide thickness: 0.325 bare,
  0.808 at 3.88 µm, 0.759 at 1 mm. Above 1500 K it is degraded by
  `exp((1500 - T_max)/300)`, no lower than 0.325 (C1104).
- Steel emissivity: `0.25617 + 3.474e-4 (T - 616.48)` (C1102).
- Debris emissivity: 0.9999.
- Steam emissivity comes from a table. Beam lengths are
  `3.5 (pitch - 2 R_clad)` for rods, and 1.8 times the gap for canisters and
  blades. Default gaps are 4, 5.05 and 14 mm.
- View factors are fixed user "exchange factors". Defaults in the 2.0
  scheme:

  | Factor | Default |
  |---|---|
  | Canister to clad | 0.25 |
  | Blade to canister | 0.25 |
  | Radial cell to cell | **0.1** (0.25 in 1.86) |
  | Axial cell to cell | **0.1** |
  | Pool or lower head to the lowest cell | 0.25 |

  An optional "MODEL" uses exponential attenuation (Eqs 2-15 to 2-31).
- Radial radiation runs to a **boundary heat structure**, the BWR shroud
  (about 5 cm). Axial radiation runs to the upper plenum.

**Creep rupture by control functions** (V1 CF §2.1.9, §2.1.10, §7.8)
- There is no built-in steam-line model.
- `PIPE-STR` is the Lamé inner-surface stress.
- `LM-CREEP` gives `t_R = 10^(P_LM/T - C)` with
  `P_LM = min(a1 log10 sigma + b1, a2 log10 sigma + b2)`. Damage sums
  `dt/t_R`, and failure is at 1.
- Table 1 defaults:

  | Material | a1 | b1 | C | a2 | b2 |
  |---|---|---|---|---|---|
  | A-508 | −5335 | 62291.3 | 16.44 | | |
  | SS-316 | −7400 | 81088.4 | 16.44 | −35555.5 | 322036.2 |
  | INC-600 | −6296 | 67129.96 | 11.44 | | |
  | SA533B1 | −4730 | 4.81e4 | 7.042 | | |

**Choked flow** (V2 CVH/FL §6.3, CVH/FL-RM-64/65)
- Checked only above 20 m/s (C4402).
- Gas: `G = rho c_s [2/(gamma+1)]^((gamma+1)/(2(gamma-1)))` with
  gamma = 1.4, using the **mixture's** sound speed.
- Liquid: extended Henry-Fauske or Moody.
- Two-phase: a void-weighted blend.
- Discharge coefficient: 1.0.

**Against `bwr_mark1`**
- No in-core radiation and no RPV-wall or internals heat structures.
- The steam-line leak is an \[A\] area opening at an \[A\] gas temperature,
  not creep.
- The choked flux is steam-only, applied to the H2-steam gas. **[inferred]**
  So the model over-vents a hydrogen-rich vessel.

---

## F8. Gases and core-concrete interaction (NCG, CAV packages)

**Gases** (V2 NCG/H2O-RM-5 to 7, 33 to 37)
- Each non-condensable is an ideal gas with its own
  `c_v(T) = cv0 + cv1 T + cv2 T^2 + cv3 T^3 + cvsqrt/sqrt(T) + cvm1/T + cvm2/T^2`
  (Eq 1-4), reference 298.15 K (C2090), and a formation energy built into
  the enthalpy.
- Library coefficients, J/(kg K):

  | Gas | Coefficients |
  |---|---|
  | N2 | cv0 1117, cvm1 −2.880e5, cvm2 5.348e7 (300–5000 K) |
  | H2 | cv0 −17849, cv1 11.28298, cv2 −2.1081958e-3, cv3 1.5635602e-7, cvsqrt 865616, cvm1 −8188058.3, cvm2 1.925734e8 (100–6000 K) |
  | O2 | cv0 1245, cvsqrt −16763, cvm1 1.111e5 |
  | CO | cv0 1116, cvm1 −2.7312e5, cvm2 4.9348e7 |
  | CO2 | cv0 1351.35, cvm1 −3.4497e5, cvm2 4.138e7 |

- **[inferred]** Molar `c_v` at 300 / 600 / 1000 K:
  - N2: 21.0 / 22.0 / 24.7;
  - H2: 20.5 / 21.0 / 21.9;
  - CO2: 29.1 / 39.2 / 46.1.
- So the model's constants (N2 20.8, H2 20.5) are within about 2 % below
  450 K. N2 is 19 % low at 1000 K.

**Core-concrete interaction** (V2 CAV §1 to §3.6 and App. A; V1 CAV-UG-10 to 13, 32)
- CORCON-Mod3.
- Quasi-steady ablation: mass flux = heat flux / heat of ablation, with the
  surface held at `T_abl` (CAV-RM-14/15).
- Equilibrium chemistry over the gases H2O, CO2, H2 and CO. Metals reduce
  the oxidising gases.
- Standard concretes, mass % (V1 Table 2.1):
  - basaltic: SiO2 54.84, CaO 8.82, CO2 1.50, bound H2O 2.00, free H2O
    3.86, …;
  - limestone/common sand: SiO2 35.80, CaO 31.30, CO2 21.15, bound H2O
    2.00, free H2O 2.70, ….
- `T_abl`: 1450 K basaltic, 1500 K limestone. Density 2340 kg/m³.
- The manual says these are not recommended values.
- **[inferred]** Gas yield: about 3.6 mol/kg for basaltic concrete and about
  7.4 mol/kg for limestone.

## F11. Combustion (BUR package)

**Basis** (V2 BUR, BUR-RM-4 to 16)
- HECTR 1.5 deflagration; detonation is only flagged.

**Ignition needs all three tests**
- Combustibles: `X_H2 + X_CO (L_H2/L_CO) >= L_H2`, with **L_H2 = 0.10** and
  L_CO = 0.167 (0.07 and 0.129 with igniters).
- Oxygen: **X_O2 >= 0.05**.
- Inerting: **X_H2O + X_CO2 < 0.55**.

**Completeness**
- `CC = 23.4116 (Y_max - 0.03746)`, with
  `Y = X_H2 + X_CO (0.08/0.148)`.
- **[inferred]** CC reaches 1 at about 8 % H2.

**Flame speed**
- Piecewise linear in Y, for example `59.2 Y + 1.792` m/s for Y up to 0.1.
- Diluent factor: `max(0.05, 1 - 4.53 X_D + 5.37 X_D^2)`.
- Burn time: equal-volume-sphere radius divided by the flame speed.

**Propagation to a neighbouring volume**

| Direction | H2 limit | CO limit |
|---|---|---|
| Upward | 0.041 | 0.125 |
| Horizontal | 0.06 | 0.138 |
| Downward | 0.09 | 0.15 |

**Against `bwr_mark1`**
- Explosion at building H2 above 8 % \[A\]: no O2 or diluent test.
- **[inferred]** A well-mixed 26 000 m³ volume at 10 % H2 burns in about
  2.4 s.

## Pressure suppression pool (CVH/FL, SPARC)

- **Opt-in.** By default gas entering below a pool surface passes through
  unchanged. Pool interaction is switched on per flow path (FL_JSW
  IBUBF/IBUBT, default 0) (V1 FL-UG-13, 25; V2 CVH/FL-RM-61).
- When it is on, the condensation efficiency is `eps = eps_z eps_T`:
  - `eps_z = (z_pool - z_junction - 0.01)/(1.0 h_junction)`;
  - `eps_T = (T_sat(P_junction) - T_pool - 0.1)/5.0`;
  - each clipped to [0, 1] (Eqs 6-1 to 6-3, C4405).
  - **Condensation stops once the pool is within 0.1 K of saturation at the
    vent exit.**
- Model: the pool condenses all steam, which is the ε = 1 limit.

---

## F9. Fission products (RN package)

**Classes** (V2 RN §2.1, Table 2.1, RN-RM-11/12)
- 17 classes by default: Xe, Cs, Ba, I, Te, Ru, Mo, Ce, La, U, Cd, Ag, B,
  H2O, Concrete, CsI, Cs2MoO4.

**Inventory** (§2.2, RN-RM-15)
- Grams of each element per watt of operating power, from Sandia-ORIGEN.

**Gap release**
- At 1173 K cladding (RN1_GAP00), for the whole radial ring (§2.3.1).

**Forms**
- On release, elements become CsOH, TeO and I2 (C7120). CsI is formed only
  when the user requests it.

**CORSOR-M** (Eq 2-11, C7102)
- Fractional rate per minute `= k0 exp(-Q/RT)`:

  | Class | k0 (min⁻¹) | Q (kcal/mol) |
  |---|---|---|
  | Xe, Cs, I2, Te | 2.0e5 | 63.8 |
  | Ba | 2.95e5 | 100.2 |
  | Ru | 1.62e6 | 152.8 |
  | Mo | 23.15 | 44.1 |
  | Ce | 2.67e8 | 188.2 |
  | La, UO2 | 1.46e7 | 143.1 |
  | Cd, Ag | 5.95e3 | 70.8 |

- Te is retained ×0.025 while unoxidised cladding is above 70 % (C7105).

**CORSOR-Booth** (§2.3.1.3, Eqs 2-12 to 2-22)
- Booth sphere release, in series with gas-phase mass transfer.
- Other classes scale from Cs by `S_k` (C7103): I 0.64, Te 0.64,
  Ba 4e-4, Ru 2.5e-3, Mo 1e-3, Ce 4e-8, UO2 3.2e-4, ….
- The default release option is ICRLSE = −5, "revised, high burn-up" (V1
  RN-UG-37). It is not documented further in V2.

**Aerosols** (§2.4)
- MAEROS sectional.
- Agglomeration: Brownian, gravitational and turbulent kernels.
- Deposition: settling, with Cunningham constants 1.257, 0.4 and 1.1;
  Brownian; thermophoresis; and diffusiophoresis with Stefan flow.

**Pool scrubbing** (§2.7.1)
- SPARC-90 decontamination factors for each aerosol bin and for I2.

**Decay heat of released fission products** (§2.6)
- Airborne: 50 % to the atmosphere, 50 % to surfaces.
- Deposited: 50/25/25.

**Against `bwr_mark1`**
- No fission products are tracked.
- Reusable pieces in this workspace:
  - `boon_lay` Booth-sphere release;
  - `changi::flexpart::aerosol`, the same Cunningham constants;
  - `outram_mc_libs::inventory::RadionuclideInventory`, with a
    Z-to-class map.

## F10. Decay heat (DCH package)

**With RN on** (V2 DCH §2.1, §2.2, DCH-RM-5 to 8)
- Per class, from Sandia-ORIGEN tables: 29 elements at 28 times to 10 days.
- The BWR base case is 3578 MWt, 3–4 years in core.
- Class powers are normalised to the whole-core value (DCH_NRM, default
  YES).

**ANS 1979 option** (§3.2, Eqs 3-1 to 3-5)
- BWR default fission-power split: U-235 64.7 %, Pu-239 31.1 %, U-238 4.2 %.
- Operating time T = 5.05e7 s.
- Capture factor `G(t) = 1 + (3.24e-6 + 5.23e-10 t) T^0.4 psi`, with
  psi = 0.713.
- U-239 and Np-239 actinide terms.

**Split of decay heat in the core** (V2 COR §2.7.2, Eqs 2-223 to 2-225)
- In the core, only the unreleased fission products heat the fuel, so heat
  leaving with volatiles is removed automatically.
- 10 % escapes the UO2 to other components in the cell (C1322(1) = 0.9).

**Against `bwr_mark1`**
- `DecayHeat::new_at_equilibrium(U235Thermal)`: the 1978 draft ANS fit,
  U-235 only, infinite irradiation, no `G(t)`, no actinide term.
- **[inferred]** Net about +5 to +10 % decay power over hours to a day if
  corrected.

---

## Contradictions inside the manuals (flagged, not resolved)

| Quantity | V2 (2023) | V1 (2017) |
|---|---|---|
| Debris fall velocity | 5 m/s (COR-RM-68) | 1.0 m/s; 5 m/s "old" (COR_LP VFALL) |
| Lower-head mechanical model, C1600(1) | 0.0 (0-D) (COR-RM-178, 248) | 1.0 (1-D) (COR-UG-206, 30) |
| Lower-head Larson-Miller slope | −4725 (C1601(1)) | −4730 (CF table) |
| CORSOR-Booth Cs: D0 / Q / grain radius | 1.0e-6 m²/s / 3.814e5 / 6e-6 m (C7106) | 2.3e-9 / 2.411e5 / 6e-6 ("2.0" default) |
| CORSOR B units | printed °C⁻¹, while Eq 2-10 takes T in K | — |

Each value used must cite the volume it comes from.

---

## What this means for the model, by priority

These are predictions to test. Signs come from the readers and are not
measured.

**Status, 2026-10-29 evening.** Each item is marked below. The measured
movements and predictions are in `docs/fukushima-melcor-gap-plan.md` §2a.

1. **Lower-head failure on a wall node, not the debris temperature.**
   - Use a wall with a 1000 W/m²K debris coefficient and 10 W/m²K outside.
     Fail at TPFAIL 1273.15 K, or by Larson-Miller creep with dP plus the
     debris weight.
   - Effect: lower-head failure **later**.
   - ~~**[inferred]** On the dry-plenum path, the current 1500 K check
     trips as soon as debris arrives (about 2100 K), so it never decides
     anything.~~ **Contradicted by measurement, 2026-10-29.** The input
     sensitivity study (`crates/sembawang/verification_and_validation/bwr_mark1/input-sensitivity.md`)
     gives `lower_head_failure_k` a stable elasticity of +0.25 on the
     lower-head time. So in the baseline run the threshold does decide:
     debris lands in water, is quenched, and then heats to 1500 K after
     the plenum dries.
   - **Implemented (gh:#1034):** inner node at TPFAIL, plus Larson-Miller
     creep. Lower head later by 0.14 h. The outer face is adiabatic rather
     than 10 W/m²K.
2. **Hold collapsed material in the core until the support fails**, and
   replace the 2100 K whole-node slump with 2400 K cladding breach, 2500 K
   or 3100 K fuel collapse, and time-at-temperature damage.
   - Effect: lower head **later**, H2 **up** (Zr stays longer in the fastest
     oxidation band).
   - **Partly implemented (gh:#1035):** breach, collapse and damage are in,
     plus Zr fusion heat and a ZrO2-melting collapse. H2 went up as
     predicted, but the lower head did not move later. **Support holdup is
     blocked** on the core plate's dimensions (L4).
3. **In-core radiation** (radial and axial 0.1, to a shroud heat structure)
   **and RPV internals heat structures.**
   - Effect: lower head **later**, H2 likely **up**.
   - **Radiation implemented (gh:#1037):** lower head later by 0.19 h, H2
     −15 %. The RPV internals heat structures are still open.
4. **Lower-plenum quench** at 100 W/m²K, with the Lipinski dryout cap.
   - Effect: lower head **earlier** (debris heats while still wet).
   - **Checked and rejected for now:** the cap is about 40–110 MW against
     about 6 MW of debris power, so it cannot bind here
     (`crates/sembawang/verification_and_validation/bwr_mark1/lower-head-wall.md`).
5. **Swollen-level coverage**, v0 = 0.3 m/s, void cap 0.4.
   - Effect: uncovery **later**.
   - **Implemented (gh:#1036).** The swell is centimetres, but it tipped the
     run into an early regime. The prediction missed; see the plan's §2a on
     regime fragility.
6. **Decay heat:** mixed-nuclide, finite irradiation, `G(t)`, actinides.
   - Effect: lower head **earlier**, H2 **up**.
   - **Implemented (gh:#1041):** decay power +6 to +11 % over 3–25 h, lower
     head earlier by 0.16 h, H2 +1 %. As predicted.
7. **Fission products:** release (CORSOR-M), with decay heat leaving with
   the volatiles.
   - Effect: lower head **later**. New output: Cs and I release.
8. **BUR ignition tests** instead of the 8 % \[A\] threshold, plus building
   O2 and diluent tracking.
   - Effect: explosion **less** reachable, unless the building is split
     into floors or CO is added.
   - **Deferred:** no hydrogen reaches the building at all yet. The drywell
     peaks at 0.56 MPa, below the 0.75 MPa \[A\] flange-leak threshold, so
     this cannot change anything until drywell pressure does (item 10).
9. **Pool condensation efficiency.**
   - Effect: drywell peak **up** once the pool nears saturation.
10. **MCCI gas, `c_v(T)`, mixture choked flow, oxidation cutoff 1100 K, a
    gaseous-diffusion limit.** Each is smaller.

**Acknowledgement.** Model descriptions are from Sandia National
Laboratories' MELCOR Computer Code Manuals, Vols. 1 and 2 (SAND2017-0455 O;
SAND2023-10994 O), cited by section and page, restated in our own words.
