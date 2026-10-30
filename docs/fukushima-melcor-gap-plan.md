# Fukushima Unit 1 simulator: closing the MELCOR gap — plan

**Status:** plan only; nothing in it is built. **Date:** 2026-10-28.
**Epic:** gh:#1028. **Gap analysis this plan answers:**
[#1028 comment of 2026-10-28](https://github.com/theodoreOnzGit/outram-park-backend/issues/1028#issuecomment-6483771300).

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). A severe-accident
> reconstruction is not a licensing input, not a PSA, not an emergency-response
> tool, and not authoritative for any facility. AI draft, untrusted until a
> human reviews it.

## 0. What this is for

The Fukushima Unit 1 simulator, ~~`crates/dhoby-ghaut/examples/fukushima_web/`~~
`sembawang::bwr_mark1` (`crates/sembawang/src/bwr_mark1/`, moved 2026-10-29 by
F2, gh:#1033; the `dhoby-ghaut` `fukushima_web` demo draws it), is
a reduced model. Against BSAF Phase II (NEA No. 7525) it misses, uncalibrated:

| | Model | BSAF Phase II, Unit 1 |
|---|---|---|
| Lower-head failure | ≈ 5.5 h | 7.45–17.16 h |
| In-vessel hydrogen | 127 kg | ≈ 400–1 310 kg *(figure-read)* |
| Peak drywell pressure | 0.53 MPa | 0.8–1.0 MPa |
| Wetwell vent, building explosion | never | ≈ 24 h, 24.83 h *(observed)* |
| Cs-137 | not computed | 5.5–43.2 PBq, three units |

This plan closes those gaps one physical mechanism at a time, measuring how much
of each miss each mechanism carries. It does **not** port MELCOR, and nobody on
it may read MELCOR source (`docs/melcor-parity-scoping.md` §1.1). Every model is
written from the public document it cites.

## 1. Rules every item follows

These are the root `CLAUDE.md` rules, restated where they bite hardest here.

1. **Right physics first; calibration only as the LAST step** (maintainer,
   2026-10-29: "focus on right physics rather than right results, u shldnt
   calibrate until the last step"). ~~The baseline is uncalibrated, and stays
   so.~~ Until every physics item (F3–F12, F14) and the input check (F13) are
   in, no item tunes a coefficient toward a BSAF band. The bands are the check;
   the moment one becomes an input the check is gone. An honestly-derived miss
   is a result. **Calibration, if it comes at all, is a separate final step**
   under the root `CLAUDE.md` model hierarchy:
   - the uncalibrated result is recorded first;
   - each adjusted parameter, its range and the dataset are named;
   - each calibrated parameter is ablated back to its derived value, one at a
     time, to show how much of the agreement it carries.
2. **Prediction before measurement.** Each item states, before it is run, which
   way it should move each Unit 1 metric and roughly how far. A change that
   moves a metric the wrong way means the hypothesis is wrong; stop and re-read
   the source.
3. **One mechanism, one ablation.** Each new mechanism lands with a
   `without_*` switch (default **on**, per the correct-physics-default rule), so
   the harness of F1 can report how much of each miss it carries.
4. **Which BSAF numbers count.** Measured (Tier A) and well-constrained (Tier B)
   quantities in `derived/nea7525-bsaf-fukushima-daiichi-bands.md` §"tiering"
   are checks. Tier C spreads are context only, and Tier D (MCCI erosion to
   500 h) is **never** a target: the report itself calls it unrealistic.
5. **Every input cites its public source** at its definition, with its
   `[P]`/`[U]`/`[A]` tag kept honest (`params.rs` convention).
6. **V&V documentation** carries methodology and measured results with dates,
   in the test's doc comment.
7. **Copyright**: see §4. The access tier comes from the document's own
   statement, never from where it is hosted.

## 2. The items, in dependency order

The issue numbers are in §3. "Expected effect" is the predicted sign on the
Unit 1 metrics. ~~none is measured yet~~: since 2026-10-29 several are; see
§2a for what each step measured against its prediction.

| Item | What | Expected effect | Needs |
|---|---|---|---|
| **F1** | Unit 1 comparison harness and ablation switches | none (measures) | BSAF bands (held) |
| **F2** | Move the Unit 1 physics into `sembawang` as library code | none intended; PETIR changes bits, re-bless with a recorded diff | ~~maintainer go-ahead~~ approved 2026-10-29 ("Physics libraries go to sembawang") |
| **F3** | Lower head: heat structure, debris-to-head heat transfer, penetration and creep failure | lower-head failure **later** | L1/L3, L10 |
| **F4** | Core: candling, continued oxidation of relocating Zr, core-plate retention and support failure | H₂ **up**; failure **later** | L1/L3, L12, L13 |
| **F5** | RPV two-phase mixture level (swell) | uncovery **later** | L1/L2, L20 |
| **F6** | RPV wall and internals as heat structures, in-core radiation, main-steam-line creep as a leak path | heat-up **slower**; leak path physical | L1/L3, L4, L10, L21 |
| **F7** | Temperature-dependent fuel and clad properties from OFFBEAT; one cited Zr–steam reaction heat | small, either sign; removes `[A]` constants | L13, L14 |
| **F8** | Multi-non-condensable containment (CO, CO₂, O₂) and MCCI gas generation | drywell pressure **up** after breach | L1/L3, L15 |
| **F9** | Fission products: release from UO₂, transport, pool scrubbing, aerosol settling; wired to `changi` | new output: Cs-137 against BSAF | L1/L3, L16, L17 |
| **F10** | Decay heat: actinide mix, irradiation history, decay heat leaving with fission products | ~~small; decay power **lower** later~~ **measured +6 to +11 % over 3–25 h** for the parts landed (§2a); ~~the part that leaves with fission products, still to come with F9, lowers it~~ landed 2026-10-29: lower head 0.055 h later, drywell peak up (§2a) | L17, L18 |
| **F11** | Hydrogen to the building, and combustion computed (flammability, steam inerting, deflagration) | building explosion becomes reachable | L19 |
| **F12** | Injection that bypasses the core, long-term containment leak, runs to 500 h | debris hotter, drywell pressure **up** | L9 |
| **F13** | Check every `[U]`/`[A]` design value and event time against open sources | corrects inputs; either sign | L4, L5, L6, L7, L9 |
| **F14** | Units 2 and 3 (RCIC, HPCI, ADS, torus stratification) | new units | `sembawang` CV EOS (`melcor-scoping.md` §3.1), L4, L9, L22 |

Order: **F1 first** (everything is measured through it), and **F13 early**,
because it is cheap and an input error can masquerade as a missing mechanism.
F3 and F4 next, as the leading candidates for the two largest misses. F14 waits
on the two-phase multi-component control-volume EOS that `melcor-scoping.md`
§3.1 names as the blocker for everything beyond a reduced model.

F3–F12 can be prototyped in the reduced model first. That says which mechanisms
carry the misses before the long `sembawang` programme starts. ~~If F2 is
approved, they land in `sembawang` directly.~~ F2 was approved 2026-10-29: they
land in `sembawang::bwr_mark1` directly.

## 2a. Progress, measured (2026-10-29)

Each step was predicted before its run, and an ablation reproduces the model
before it exactly. Inputs come from the MELCOR 2.2 manuals' public
description (`crates/kovan-literature/derived/melcor-2.2-manuals-digest.md`),
MATPRO and ANS-5.1-1979, as each commit cites. **Nothing was tuned.**

| Step | Item, issue | Lower head (h) | H2 at 25 h (kg) | Prediction |
|---|---|---|---|---|
| Baseline (F1) | #1032 | 5.391 | 126.62 | — |
| MATPRO c_p | F7, #1038 | 5.409 | 124.15 | held |
| Reaction heat 6.763 MJ/kg | F7, #1038 | 5.386 | 122.31 | held |
| H2 sensible heat paid | #1046 | 5.399 | 120.73 | held |
| Lower-head wall | F3, #1034 | 5.543 | 119.95 | sign held, size missed |
| Zr fusion heat | F4, #1035 | 5.413 | 180.57 | lower-head sign missed |
| Rod collapse | F4, #1035 | 5.395 | 216.06 | H2 held, lower head missed |
| In-core radiation, shroud | F6, #1037 | 5.589 | 182.90 | held |
| Swollen level, ZrO2 melting | F5, #1036 | 4.668 | 283.76 | missed: regime change |
| ANS-1979 decay heat | F10, #1041 | 4.509 | 286.63 | held |
| H2-steam mixture choking | #1037 | 4.509 | 287.47 | held |
| N2 and H2 `c_v(T)` (MELCOR NCG fits) | F8, #1039 | 4.509 | 287.47 | drywell sign missed (+1.5e-5 relative, predicted down) |
| Gaseous-diffusion limit on oxidation (COR Eq 2-168) | F4, #1035 | 4.484 | 292.44 | where it binds held (steam fraction < 3 %); H2 sign missed (predicted slightly down) |
| Oxidation cutoff 1000 K \[A\] -> MELCOR TOXMIN 1100 K \[P\] | F13, #1044 | 5.390 | 192.11 | missed: regime flip (late regime, no vent); bimodal in the cutoff |
| H2 `c_p(T)` in the core and vessel (MELCOR NCG fit) | F7, #1038 | 5.393 | 193.97 | lower head held (later, slightly); H2 sign missed (predicted down) |
| Main-steam-line creep rupture (PIPE-STR, LM-CREEP) | F6, #1037 | 5.454 | 191.47 | held: ruptures 4.36 h, RPV depressurised at it (was at the lower head); drywell +2.4 %, less than predicted |
| Dry-vessel outflow capped at the equalising amount (defect) | — | 5.449 | 191.43 | defect fix, found by a crash |
| Dry-vessel steam leaves at its own enthalpy (defect) | — | 5.449 | 191.43 | defect fix, found by the same crash; drywell only moves |
| Steam dome internals as a heat structure | F6, #1037 | 5.639 | 215.36 | missed: predicted no rupture; the line ruptures at 4.78 h. Rupture rests on \[A\] inputs (none at 3x area) |
| Lower-head penetrations fail at TPFAIL | F3, #1034 | 5.452 | 215.36 | held: 0.19 h earlier (predicted 0.1-0.3 h) |
| Volatile fission-product release, CORSOR-M (output only) | F9, #1040 | 5.452 | 215.36 | in-vessel release 0.85 at 25 h; predicted 0.4-0.7, missed high (BSAF: most codes > 0.8) |
| Noble-gas (Xe) transport, fuel to stack (output only) | F9, #1040 | 5.452 | 215.36 | conserved to 1e-12 every step; all 0.85 in the wetwell gas at 26 h (no vent or flange leak opens) |
| A failed vessel's gas leaves through the breach | — | **5.452** | **215.36** | drywell +1.5e-5 MPa; predicted +0.01 from post-failure H2, missed: almost no H2 is made after failure |
| Cesium and aerosol through the containment (sectional aerosol, TRAP-MELT2; output only) | F9, #1047 | 5.452 | 215.36 | conserved to 1e-9; Cs in S/C water inside the 9-code envelope 85 % of the time, on drywell structures 47 % (deposit re-evaporates) |
| Iodine through the containment (output only) | F9, #1047 | 5.452 | 215.36 | all I2 stays airborne; inside the Fig 2.6b/d envelopes 0 % |
| Pool scrubbing by steam condensation (SPARC-90's DF_EC; output only) | F9, #1047 | 5.452 | 215.36 | I in S/C water inside 54 % (was 0 %); Cs 61 % (was 85 %, reaching the pool earlier); predictions held |
| In-vessel deposition of Cs and I vapour on the dome internals (output only) | F9, #1047 | 5.452 | 215.36 | holds up to 19.4 kg Cs, then re-evaporates (internals at 1005 K, no heat-loss path: stated artefact); Cs in S/C water inside 64 % (was 61 %); prediction (0.4–0.7) missed |
| Dome internals lose heat to the wet dome (natural convection, 5 W/m²K \[A\]) | F6, #1037 | **4.469** | **295.45** | missed: regime flip to the early regime (vent flows again at 23.73 h); internals keep 11.3 kg Cs; ~~exposed a vessel-emptying defect (filed)~~ **corrected:** the emptying was the unguarded heat loss itself, removed by the wet-only limit (gh:#1051, pinned) |
| Condensation scrubbing stops near saturation (MELCOR `X_o`, Eq D-6; output only) | F9, #1047 | 4.469 | 295.45 | held: less scrubbed; I in S/C water inside 55 % (was 54 %), vented I +9 % |
| Impaction and swarm-rise scrubbing at the downcomers (SPARC-90 App. D, E; output only) | F9, #1047 | 4.469 | 295.45 | Cs fractions unmoved to 1e-3; vented Cs up 3.9 -> 6.4 kg, sign missed; hypothesis: lumped (not sectional) Cs on aerosol |
| Cs and I on the aerosol tracked by section (condensed by mass transfer, carried through agglomeration; output only) | F9, #1047 | 4.469 | 295.45 | more Cs airborne, held: vented 6.37 -> 7.93 kg; Cs on dw structures inside 33 % (was 36 %); the swarm's effect on vented Cs falls 2.46 -> 0.94 kg, sign still positive (prediction <= 0 missed); cause confirmed by switching off wetwell agglomeration (effect -1.51 kg): the swarm removes the large particles that would scavenge the small Cs-bearing ones |
| Iodine partition limit on condensation scrubbing (SPARC-90 pool chemistry, Eq D-7; output only) | F9, #1047 | 4.469 | 295.45 | held: binds almost nowhere (CsOH keeps the pool alkaline); I in pool -0.0006 kg (-0.008 %) |
| Wetwell vent 0/0 once its non-condensables are gone (defect) | #1053 | 4.469 | 295.45 | NaN wetwell from 73.5 h of the 500 h run; guarded, no metric moves; the steam-free vent itself is #1053 |
| Wetwell vent passes the gas mixture at its density; boiled-off water takes its sensible heat (defect) | #1053 | 4.469 | 295.45 | 26 h metrics unmoved; drywell +0.021 MPa at 24 h, then converges; 0-500 h against Fig 2.3a mean 0.662 -> 0.167 MPa (the 0.97 MPa plateau was this defect, not a missing leak) |
| Thermophoresis and diffusiophoresis onto the drywell structures (MELCOR defaults; output only) | F9, #1047 | 4.469 | 295.45 | held: more Cs on dw structures while a deposit exists; inside the Fig 2.6a envelope 39 % (was 33 %) |
| Swarm-rise scrubbing of I2 vapour at the downcomers (SPARC-90 App. F; output only) | F9, #1047 | 4.469 | 295.45 | held: I in pool 7.79 -> 9.41 kg, vented 1.79 -> 0.16 kg; Fig 2.6d inside 53 % (was 55 %): further above the highest code |
| Aerosol dynamics in the vessel (sectional volume, internals and floor as surfaces; output only) | F9, #1047 | 4.469 | 295.45 | missed: predicted retention in the vessel, but the in-vessel aerosol takes the CsOH vapour and carries it out; internals hold 0.05 kg at 26 h (was 11.3); Cs in pool 114 kg (was 102); Fig 2.6a inside 41 % (was 39 %) |
| Condensed Cs and I add to their aerosol section's mass (MELCOR RN §2.5.2; output only) | F9, #1047 | 4.469 | 295.45 | mostly held: vessel keeps 5.8 kg, vented Cs 8.8 -> 0.4 kg, dw structures 60 kg at 12 h (overshoot); pool up (missed); Fig 2.6a inside 23 % (was 41 %) |
| Decay heat of released noble gases, alkali metals and halogens follows them (class shares from an ENDF/B-VIII summation, 0.96-0.99 of ANS) | F10, #1041 | **4.524** | **304.26** | lower head 0.055 h later (held); drywell peak 0.595 -> 0.613 MPa; measured drywell mean miss 0.289 -> 0.274 MPa; Fig 2.6a inside 37 % (was 23 %) |
| Tellurium transported (MELCOR class 5, 15.4 kg \[A\] from the summation's Te/Cs ratio), its heat following it | F9/F10, #1041 | **4.517** | **305.64** | no prediction written (lapse, recorded); lower head 0.007 h earlier; drywell peak 0.613 -> 0.609 MPa; Fig 2.6a inside 51 % (was 37 %) |
| Lower-plenum debris oxidises while quenched (MELCOR particulate area 6V/D, D = 1 cm \[A\]) | F4, #1035 | **4.483** | **313.43** | H2 +2.5 % (predicted +10-40 %, missed); lower head 0.034 h earlier (held); D sweep non-monotonic (0.5 cm 246 kg, 2 cm 305 kg): H2 at 25 h is set by timing; measured drywell mean miss 0.274 -> 0.260 MPa |
| Candling: molten Zr flows down once the shell breaches, refreezes below (simplified MELCOR §3.1) | F4, #1035 | **5.314** | **441.97** | missed in size (predicted H2 +0-20 %, lower head within 0.2 h): H2 +41 %, now **inside its band**; lower head +0.83 h; drywell peak 0.705 MPa; measured drywell mean miss 0.260 -> 0.189 MPa; I in S/C water inside 4.5 % (was 52 %: release 0.99) |
| MELCOR falling-debris quench (100 W/m²K over the particle area, 360 s decay factor, Lipinski dryout) replaces the 120 s \[A\] quench | F3/F4, #1034 | **4.736** | **395.54** | missed (predicted within 0.1 h): lower head 0.58 h earlier, the debris keeping its decay heat under water; measured drywell mean miss 0.189 -> 0.158 MPa; RPV envelope 7 -> 35 %; H2 just below its band |
| Lower head conducts from the debris it touches under water (MELCOR COR §6.1); penetrations heated only over the debris-covered part of their height (`dz_d / dz_1`, 0.5 m bottom level \[A\]) | F3/F4, #1034 | **4.556** | **253.13** | held (predicted the head earlier by up to 0.3 h: 0.18 h); no H2 prediction written; without the coverage the head fails at the first debris (3.55 h). Measured with the drain below: drywell mean miss 0.158 -> 0.263 MPa (worse), RPV envelope 35 -> 4 %, H2 envelope 18 -> 9 %. Leans on the missing core-plate holdup (L4) |
| A failed vessel's water drains through the breach (0.1 m \[A\]) to the pedestal; a node candled bare no longer divides 0/0 (defect) | F3, #1034 | 4.556 | 253.13 | missed (predicted drywell peak up 0.02-0.1 MPa): 0.58922 -> 0.58863 MPa; the no-radiation ablation no longer goes NaN after a wet failure |
| Wetwell atmosphere with its own temperature and steam: MELCOR's pool-surface exchange (CVH/FL RM §5.1) and bubble model (ε_z ε_T, §6.1), NCG/H2O default properties | F12, #1053 | 4.556 | 253.13 | missed (predicted drywell peak within 0.01 MPa): 0.58863 -> 0.56194 MPa; with no gas bubbling after about 8 h the gas lags the pool (RH about 0.87 at 22 h), wetwell 22 kPa lower; measured drywell mean miss 0.263 -> 0.275 MPa; 0-500 h mean 0.1672 -> 0.1677 MPa |
| Drywell structures as MELCOR's heat-structure surface: natural convection (HS Table 2.3) and diffusion-limited condensation (Eq 2-136), latent heat into the wall, replacing a fixed 20 W/m²K \[A\] | F6, #1037 | 4.556 | 253.13 | inside the predicted range (-0.02 to +0.06 MPa), sign missed: drywell peak 0.56194 -> 0.56080 MPa, vent 23.73 -> 23.89 h; measured drywell mean miss 0.275 -> 0.278 MPa; Cs on dw structures inside 80 -> 83 % |
| Torus shell above the pool as a heat structure on the wetwell atmosphere (1353 m², 7.96e7 J/K \[A\], adiabatic outside) | F6, #1037 | 4.556 | 253.13 | peak held (set by the rupture disk: unchanged to 1e-6 MPa); vent 23.89 -> 24.01 h (predicted within 0.1 h: missed by 0.03 h); measured drywell mean miss 0.278 -> 0.282 MPa |
| Gas leaving the drywell (downcomers, flange leak) takes its enthalpy, MELCOR's donor convention (CVH/FL RM Eq 6-6) (defect) | F12, #1053 | 4.556 | 253.13 | missed (predicted peak within 0.002 MPa, vent later): peak 0.56080 -> 0.56641 MPa, vent 24.01 -> 23.73 h (the order's time); the pool gets each mole's `R T`, 2 K warmer; drywell gas 43 K cooler; measured drywell mean miss 0.282 -> 0.279 MPa |
| Relief valves' hydrogen reaches the wetwell with its enthalpy at the vessel's saturation temperature (energy closure, defect) | F12, #1053 | 4.556 | 253.13 | held (predicted +0 to +0.002 MPa): peak 0.56641 -> 0.56659 MPa; BSAF figures unmoved within their pins |
| Water condensed in the drywell drains to its floor (the pedestal's pool) instead of leaving the model (defect) | F12, #1053 | 4.556 | 253.13 | held (predicted +0.02 MPa): peak 0.56659 -> 0.58660 MPa; 12 t on the floor at head failure, walls 60 K cooler at 22 h; measured drywell mean miss 0.279 -> **0.263 MPa** |
| RPV wall (cylinder and top head, 0.16 m steel \[A\], 424 t) as a lumped heat structure on the saturated contents, 1 W/m²K insulation to the drywell | F6, #1037 | **4.624** | **228.61** | all missed (predicted no change before failure): the wall buffers the isolation condenser's 3.5-7.9 MPa swings in the first hour and absorbs 1.5 GJ as the vessel repressurises; head 0.07 h later; drywell peak 0.58660 -> 0.57524 MPa; measured drywell mean miss 0.263 -> 0.274 MPa |
| Dome internals radiate to the RPV wall (MELCOR gray pair, HS Eq 2-96; default steel emissivity, COR C1102; 78.4 m² envelope \[A\]) | F6, #1037 | 4.622 | 227.76 | integral metrics held (head -0.001 h, H2 -0.85 kg, peak -0.0008 MPa); internals 812 K at 26 h (1332 K without; predicted below 800 K: missed); Cs on them unchanged (predicted more: missed) |
| Top of each ring radiates to the dome internals, its axial boundary structure (MELCOR COR RM §2.1.2, Fcell,a; through emptied cells) | F6, #1037 | 4.623 | 226.81 | head and H2 held; peak 0.57458 -> 0.56772 MPa (predicted within 0.005: missed): after the slump the standing fuel's ~250 kW decay heat goes up to the internals and out through the wall, not with the debris; UO2 5.56 -> 6.17 kg; measured drywell mean miss 0.274 -> 0.276 MPa |
| Unit 1 Cs-137 release to the environment against BSAF Fig 5.1 and Table 5.1 (instrument, output only; Cs-137 share 0.3167 from the ENDF/B-VIII summation, biased low by its missing capture) | F9, #1040 | 4.623 | 226.81 | missed (predicted 0.1-1.3 PBq, envelope 20-60 %): **0.080 PBq** by 500 h, nearly all in the vent's first hours, below every code (0.1-18.9) and a tenth of GRS's 0.79; rate inside the envelope 7.3 %. After the vent the pool holds the cesium; the codes keep releasing (building leaks, F12's late leakage, revaporisation carried out by flows; here revaporisation is modelled but limited by the failed vessel's slow gas outflow) |
| Unit 1 release as a `changi` source term (`bwr_mark1::source`): hourly windows of Cs-137, I-131, Xe-133 from the summation's inventories (Cs-137 scaled to BSAF's cesium mass; I-131, Xe-133 at equilibrium), padded by `chain::pad_for_dispersion` | F9, #1040 | 4.623 | 226.81 | hand-off only (the dispersion run needs Fukushima meteorology, not held). Inventories 1.565e17, 1.358e18, 2.895e18 Bq; released by 30 h 7.96e13, 5.54e15, 2.53e18 Bq |
| Unit 1 release carried downwind (`bwr_mark1::source::near_field`): `changi`'s Gaussian puff under ASSUMED weather (3 m/s, Pasquill D, 120 m stack, held toward the receptors for 30 h \[A\]); time-integrated air and dry deposition at 1-5 km on the centreline. Illustrative chain demonstration, no dose, no comparison with measurement (assumed weather, ~6 km reach) | F9, #1040 | (output only) | (output only) | predicted by hand Gaussian before the run, every number held within its factor of 3: 4 km peak, chi/Q 1.72e-6 s/m³; Xe-133 4.34e12, I-131 4.43e10, Cs-137 1.34e8 Bq s/m³ at 4 km; 1 km 32x below 3 km (50x predicted) |
| Core radiation looks through emptied cells to the next standing component (MELCOR COR RM §2.1.2) | F6, #1037 | 4.623 | 226.85 | held (head +0.0003 h, H2 +0.04 kg, peak -0.00006 MPa); UO2 6.17 -> 6.60 kg (no prediction written); Cs-137 to the environment 0.0797 -> 0.0785 PBq |
| Molten core-concrete interaction: reduced MELCOR CAV (CORCON basaltic concrete \[A\] as Unit 1's, quasi-steady ablation at 1450 K, h = 1000 W/m²K over 21.2 m² \[A\], H2O and CO2 reduced by the debris's Zr, film boiling under water, radiation when dry) | F8, #1039 | 4.623 | 226.85 | held (predicted peak +0.05 to +0.2 MPa): peak 0.568 -> **0.774 MPa**, flange leak opens ~23 h; 41 t ablated, 271 kg H2 and 395 kg CO by 25 h; measured drywell mean miss **0.276 -> 0.153 MPa**; Cs-137 at 500 h 0.08 -> 17 PBq through the open flange, driven by the drywell structures' missing outer heat path (pre-existing: the drywell gas reaches 1000 K by 480 h with or without MCCI) |
| Drywell structures' outer heat path: across the shell-to-concrete gap (gas conduction plus gray radiation) into a 1-D 1.5 m concrete slab to the reactor building (all \[A\]) | F6/F12, #1037 | 4.623 | 226.71 | held: peak 0.774 -> 0.688 MPa (predicted down 0.03-0.10), the flange stays closed; drywell gas at 480 h 1008 -> 420 K (predicted below 600); Cs-137 by 500 h 17 -> 0.059 PBq (predicted below 1); measured drywell mean miss 0.153 -> 0.198 MPa |
| BWR core plate holds relocating debris until it yields (MELCOR PLATEB, COR RM §5.2.3, Eqs 5-3, 5-13, 5-16; 0.05 m on 0.3 m beams \[A\]) | F4, #1035 | **5.000** | **183.59** | all missed (predicted head +0-0.3 h, H2 within 20 kg, peak within 0.02): head +0.38 h, H2 -43 kg, peak 0.688 -> 0.641 MPa; the first debris held dry no longer quenches in the lower plenum, so the core is steam-starved (H2 stays at 75 kg, 3.67-4.99 h); measured drywell mean miss 0.198 -> 0.236 MPa |
| The bottom of the core radiates to the lower plenum's water (MELCOR COR RM §2.1.2, `FLPUP` 0.25 \[P\], water emissivity 0.95 \[A\]; blocked while debris sits on the plate) | F2, #1035 | **5.983** | **296.36** | all missed (predicted head 0.05-0.3 h earlier, H2 +20-80 kg, peak within 0.03): the mechanism is small (0.21 MW from the whole core at 1500 K) but the head failure at 5.00 h was a knife edge (the runs agree to 18000 s; the old head fails at 943 K, the new one is at 915 K as the relief valves cycle); the debris then quenches, H2 +113 kg, the main steam line creeps through at 5.24 h and the head fails dry at 5.98 h; peak 0.641 -> 0.561 MPa; measured drywell mean miss 0.236 -> 0.264 MPa. Head-failure time not resolved better than ~1 h by this model |
| One core plate per radial ring, each failing on its own load (MELCOR COR RM §5.2.3: the supporting structure is a component of each ring's cell) | F4, #1035 | **4.590** | **253.50** | three of four missed (predicted the first plate 0.1-0.5 h earlier, the late branch kept with the head at 5.5-6.5 h, H2 +0-60 kg, peak within 0.05): ring 1's plate holds the first 2.4 t on a third of the area and fails at 4.47 h (0.52 h earlier); that slug fails the head in wet contact at 4.59 h, the early branch; rings 2 and 3 fail at 4.90 and 5.42 h; H2 -43 kg, peak 0.561 -> 0.755 MPa (measured 0.84 MPa), past the flange leak (0.75 MPa \[A\]): 9e-4 of the Xe reaches the building; measured drywell mean miss 0.264 -> 0.148 MPa |
| The torus pool in two layers split at the downcomers' exits (1.21 m from the submergence head; prismatic pool, stated): downcomers and surface processes act above, the relief valves' quenchers below, the layers mix when the lower is hotter; no conduction between them (stated). Physics from the PPOOLEX experiments and Li and Kudinov's EHS/EMS models | F14 (torus stratification, taken up for Unit 1), #1037 | 4.591 | 256.41 | predicted first: no change before the head fails (held); surface 380-400 K at 11.7 h (378 K at 12 h; mixed 356 K); drywell +0.1-0.2 MPa at 11.7 h (+0.084, low); peak 0.80-0.95 (0.965, just above); flange open before 20 h (held, ~17.5 h); building H2 < 4 % (held, 1.9 %); in-vessel H2 unchanged (+2.9 kg, missed). The model now rises past the measured plateau after ~16 h; measured drywell mean miss 0.148 -> 0.119 MPa; Cs-137 to the environment 0.060 -> 0.154 PBq, inside the codes' 0.1-18.9 PBq for the first time |
| The debris on the core plates oxidises in the steam rising past it (MELCOR COR §3.2.1, particulate debris), which then no longer reaches its ring's core | F4, #1035 | 4.591 | 252.87 | predicted first: H2 +0-15 kg (sign missed: -3.5 kg), head within 0.1 h (held: unchanged), peak within 0.02 MPa (held: +0.0006); the starved core would have used that steam anyway, so the H2 moves lower rather than growing (likely; not separately measured) |
| Conservation defect fixed: the lower plenum's debris oxidation takes its steam out of what rises into the core (the core could use it a second time) | F4, #1035 | 4.591 | 247.99 | predicted first: H2 down 0-10 kg (held: -4.9), head within 0.02 h (held: unchanged), peak within 0.01 MPa (held: -0.0003) |
| The control blades, MELCOR's NS component (steel only): 97 x 60 kg \[U\]/\[A\] at each cell's cladding temperature, MELCOR's stainless-steel oxidation (COR RM §2.5.6, Tables 2-5, 2-9, MP_SS defaults), failing at 1700 K and draining to the lower plenum where the steel quenches; B4C not modelled | F4, #1035 | **5.451** | **474.93** | predicted first: heat-up slower (held), H2 within 25 kg (missed: +227) and up 5-40 kg by 4 h (missed: +0.2), head 4.2-6.2 h (held); the steel's own oxidation gives ~10 kg H2; the 5.8 t of blade steel quenching in the plenum's water from ~4.3 h raises ~2.5 t of steam that ends the steam starvation (Zr oxidised 6.4 -> 10.6 t); H2 now inside the codes' 398-1315 kg; drywell peak 0.965 -> 0.820 MPa |
| The blade's heat of fusion (MELCOR MP default 2.68e5 J/kg): held at 1700 K while melting, drained with the heat | F4, #1035 | 5.240 | 449.03 | predicted first: H2 +20-80 kg (sign missed: -26), head within 0.3 h (held: -0.21), peak within 0.05 MPa (held: -0.005); the hold slows the hottest nodes and delays the drain (likely) |
| The cavity debris's phases (MELCOR CAV RM §3.3, §3.10): ideal-solution pseudo-binary melting range (fuel UO2+ZrO2 against the concrete's oxides as SiO2 \[A\]) with latent heats; interior conducting (6k/H, k 2.5 \[A\]) when solid, convecting when liquid | F8, #1039 | 5.240 | 449.03 | predicted first: cavity H2 50-400 kg (held: 265, with 385 kg CO), drywell H2 50-300 kg (held: 100-140), flange earlier (held: 16.7 h), in-vessel unchanged (held); drywell +0.05-0.3 MPa over 10-24 h (partly missed); building H2 0.5-6 % (6.8 %, just above). 71 t of concrete ablated by 25 h; drywell plateau 0.70-0.75 MPa from 16 h (measured ~0.75); measured drywell mean miss 0.109 -> 0.078 MPa |
| The building's burn by MELCOR's BUR criteria without igniters (BUR RM §2.2: Le Chatelier with CO, O2 >= 0.05, steam + CO2 < 0.55), CO and CO2 followed as tracers in the N2-like gas; replaces the 8 % H2 threshold \[A\] | F11, #1042 | 5.240 | 449.03 | predicted first: no ignition, run unchanged (held, byte for byte); the building's Y peaks at 0.0714 (H2 0.0685, CO 0.0049) at 26 h, below 0.10; it passes the with-igniters 0.07 between 24 and 26 h (the explosion was at 24.8 h: recorded, not adopted) |
| The wetwell vent and the drywell head flange limited to the critical gas flow (MELCOR CVH/FL RM §6.3, Eq 6-13): a missing limit, the incompressible orifice passing about twice it at those pressure ratios | F12, #1043 | 5.240 | 449.03 | predicted first: peak 0.77-0.83 MPa (held: 0.804), building H2 at 26 h 3.5-5 % (3.28 %, narrowly missed), drywell at 24 h 0.25-0.4 MPa (0.198, missed), in-vessel unchanged (held); measured drywell mean miss 0.078 -> 0.067 MPa, inside the codes' envelope 58 -> 71 % of the time |
| The blades' steel oxidation held to MELCOR's gaseous-diffusion limit (COR RM §2.5.3, Eq 2-168; MW/n 0.0505 kg/mol) | F4, #1035 | 5.238 | 448.16 | predicted first: H2 within 10 kg (held: -0.87), head within 0.05 h (held: -0.002), peak within 0.01 MPa (held: -0.0006) |
| **Perturbation ensemble** (instrument, `compare::ensemble_member`, 256 members at rel 1e-9; `verification_and_validation/bwr_mark1/ensemble.md`): the run is chaotic at the 1e-9 level, so a single run is one trajectory | #1044 | median 5.23 (IQR 4.91-5.24) | median 449 (IQR 417-449) | predictions: head range narrower than predicted (4.50-5.56), baseline inside the IQR (held); H2 305-481 kg (held); peak up to 0.97 MPa (missed: 0.75-0.87); vent at 23.73 h in all (held). Quote these distributions, not the single run |
| Checked against the manual, no change (2026-10-30): **debris ejection** (COR RM §6.4): once the bottom node passes TPFAIL MELCOR assumes gross failure and discharges all the bottom cell's debris over 1 s regardless of the opening, which is what this model does at its TPFAIL head failure. **Radial relocation** (§3.1.5, on by default): MELCOR blocks it while a BWR canister stands in either ring and to or from any cell with a plate, which covers every debris location here. **UO2 carried by molten Zr** (§2.8 eutectics, §3.1.4 secondary transport): ~~optional models, not defaults, so not added.~~ **CORRECTED 2026-10-30:** the eutectics model (COR_MS IEUMOD) is off by default and not recommended, but COR_CMT's secondary transport applies by default (FUOZR = 0.2, mechanism 1: UO2 relocates as 0.2 of the candling Zr's mass; the record is optional, its defaults are not); added below. **Cavity layering** (CAV §3.2): MELCOR's default is enforced mixing, one layer, as here; stratification is an option, not added. **B4C oxidation**: MELCOR's default BWR model caps it at 2 % (about 4 kg of H2 here), not added | F4, #1035 | - | - | upstream handles these as this model does, or as options |
| Numerical defect fixed: the downcomers' and vacuum breakers' equalising cap counts the receiver's rise as well as the donor's fall (it overshot, and the vacuum breaker chattered at its set point every step from 2.55 h, amplifying perturbations 1.5x a step) | #1044 | 5.404 | 479.93 | predicted first: chatter gone (held), 1e-9 sensitivity down 100x (missed: still 5.4e7; first divergence now at 3.49 h, a core threshold event), single-run move inside the ensemble spread (held). Read single-run moves against the ensemble |
| The blades' B4C by MELCOR's simple model (COR RM §2.5.9, MARCON 2.1B: Eq 2-208 rate, 2 % cap, equilibrium CO/CO2/CH4 split, Eqs 2-205-2-207 energies); 4.7 kg per blade \[A\] | F4, #1035 | 5.343 | 459.87 | predicted first: 1.8-2.3 kg H2 mostly as CO (missed: 2.6 kg, mostly CO2), ~165 mol carbon gases (held: 160), single-run move inside the ensemble spread (held: a branch change) |
| Building burn dynamics, MELCOR's BUR (RM §2.3-2.5): the HECTR 1.5 completeness (Eqs 2-8, 2-9) and flame speed (Eqs 2-10-2-19), duration `CDIM / V` with BUR_BRT's default CDIM, rate Eqs 2-21-2-23; oxygen consumed and CO2 made; pressure rise reported as an adiabatic constant-volume bound (the building is held at 1 atm) | F11, #1035 | 5.343 | 459.87 | predicted first: the baseline does not ignite (Y peaks 0.033), so byte-identical (held: the fixture passes unchanged); hand values for the rates, completeness and a 12 % burn's 0.47 MPa bound (held) |
| Debris falling through the pedestal's water, MELCOR's FDI low-pressure melt ejection (RM §2.2): COR Eq 6-34 velocity through the 0.1 m breach, Chu's modified breakup (Eqs 2-11-2-14) while molten, radiation or Bromley film boiling (Eqs 2-15-2-17), all heat boiling water (Eq 2-20); HPME (above 10 m/s, C4602) flagged, not modelled; time of flight within a step not modelled | F8, #1035 | 5.343 | 459.87 | predicted first: 0.05-0.5 t steam for ~2800 K molten debris (missed low: the debris leaves solid at 1350-2044 K, 63 kg by 7.1 h); byte-exact ablation; drywell peak 0.78578881 -> 0.78577510 MPa only |
| UO2 carried by candling Zr, MELCOR's default secondary transport (COR RM §3.1.4, Eq 3-14; COR_CMT MTUOZR 1, FUOZR 0.2): 0.2 of the molten Zr's mass from the node's fuel, refreezing with the Zr (joins that node's fuel, stated), counted once per flow; per-node UO2 now a state | F4, #1035 | 5.484 | 470.75 | predicted first: 0.3-2 t moved (missed high: 3.98 t, nearly all metal candles; a first draft counting every node hop gave 10.6 t, a defect fixed before measuring), head 4.9-5.4 h (missed: 5.48 h), no node emptied (held: 0.94-1.62 of its fuel); UO2 conserved to 1e-9 |
| Lower-plenum molten pool, MELCOR's convecting oxide pool (COR RM §2.4.2): above the UO2-ZrO2 eutectic (2800 K, Table 2-12) the debris-to-head coefficient is ACOPO's `Nu = 0.3 Ra_i^0.22` (Table 2-4, j = 1) with the transient Ra_i (Eq 2-126); melt properties \[A\]; no metallic pool, stratification (an option in MELCOR), Bonnet profile, Stefan substrate or in-core pools | F3, #1035 | 5.484 | 470.75 | predicted first: no pool forms before the head fails (held: byte-identical); the hand value 1609 W/(m^2 K) at 1 MW/m^3, 0.5 m (held) |
| Checked against the manual, no change (2026-10-30): **HPME / DCH** (FDI RM §2.3): MELCOR disables it unless the user supplies FDI_CVH and FDI_DEP (transport fractions, deposition surfaces), treating every ejection as LPME (FDI UG §1), which is what this model does; it also never ejects above 10 m/s here (`fdi_hpme` stays false). **Eutectics** (COR §2.8, IEUMOD): off by default and not recommended; its default proxy, COR_CMT's UO2 transport, is added above. **Cavity layers** (CAV §3.2): enforced mixing, one layer, is the default, as here. **In-core molten pools**: no fuel node reaches the 2800 K eutectic in this run (UO2 released 2.7 kg), not added | F3, F4, F8, #1035 | - | - | upstream defaults handled as this model does |

**F9 beyond noble gas, scoped 2026-10-29.** Cs and I travel as aerosol.
MELCOR gives **no default size** for a released aerosol source ("currently
there is no default", Vol. 1 RN1_AS01): the size emerges from MAEROS's
condensation and agglomeration (sections 0.1–50 µm, RN1_ASP). A reduced model
with one assumed size would let that \[A\] size decide drywell deposition and
pool scrubbing, the same trap as choosing Unit 1's concrete. Doing it
properly means porting a sectional aerosol model (MAEROS-type) and SPARC-90;
reuse to check first: `changi::flexpart::aerosol` (Cunningham slip, settling).
Gap release needs nothing: MELCOR's default gap inventory is zero (RN1_GAP
`NMGAP` = 0), which is the model's. Reference curves are digitised (Figs
2.6a–d).

BSAF bands: lower head 7.45–17.16 h (B); H2 400–1310 kg (C). ~~The vent now
flows at 23.73 h, inside its Tier A band (23.5–24.68 h), as a by-product of
F10.~~ **CORRECTED 2026-10-29:** since the 1100 K oxidation cutoff the run is
in the late regime and the vent is not reached. A diagnostic scan of the
cutoff (not a tuning run; the value is MELCOR's, chosen before the scan) is
bimodal, with flips both ways: 1000–1025 K early (4.48–4.51 h, 287–292 kg,
vent at 23.7 h), 1050–1125 K late (5.35–5.40 h, 188–193 kg, no vent), 1150 K
early again. The vent's earlier agreement was a property of the regime, not
of the vent physics.

**Against the digitised figures (2026-10-29,
`crates/sembawang/tests/bwr_mark1_vs_bsaf_figures.rs`):** the RPV was
measured at 7.00 MPa at 5.34 h, where the model, its steam line already
ruptured by creep at 4.78 h, is at 0.29 MPa: the measurement contradicts the
rupture's timing, which rests on \[A\] inputs. The drywell runs 0.24–0.51 MPa
below the 33 measured points over 10–23 h, with or without that rupture. Diagnosed the same day: the drywell rides
the wetwell, whose 344 K pool (about 100 K subcooled) condenses all steam, so
the pressure is the non-condensable gas alone; a heat sink a tenth the size
moves it 0.02–0.03 MPa; about 6e5 mol of non-condensable gas is missing, the
size of the MCCI gas BSAF's codes relied on (p.19). **F8 (MCCI) is therefore
the lever on the drywell**, and it stays blocked on Unit 1's concrete. What exactly
is missing, checked 2026-10-29: MELCOR's manuals do tabulate the standard
concretes (Vol. 1, CAV Tables 2.1 and 2.2: compositions, density, solidus,
liquidus, ablation temperature), but (1) **the type decides the gas** — CO2
is 1.5 % of CORCON basaltic against 35.7 % of limestone/common sand — so
choosing one without a Unit 1 source would be choosing the drywell answer,
which rule 1 forbids; and (2) **the heat of ablation is computed internally**
from the species' thermochemistry, not given as a number. Needed: Unit 1's
concrete type (or composition) from a plant or BSAF-specification source. H2
is below all nine codes from about 5 h; fuel release is inside the iodine
envelope 94 % of the time.

**What the run showed:**
- Three changes running, lower-head failure followed **plenum dryout**.
- Since F5 the outcome is **regime-fragile**. A large debris slug quenching in
  the wet plenum flips the run between an early regime (about 4.6 h) and a
  late one (about 5.6 h). The flip happens under small changes (bubble-rise
  scan, ablations).
- So no lower-head comparison is robust until the slug's size stops deciding
  the run: candling and support holdup (F4), which need the core plate's
  dimensions (L4).
- **H2 at 25 h is not smooth in the inputs**
  (`verification_and_validation/bwr_mark1/input-sensitivity.md`).
- The Lipinski dryout cap was **checked and rejected for now**: it cannot bind
  here (`lower-head-wall.md`).
- **Pool condensation efficiency (MELCOR's SPARC-style ε_T): relevant only
  after the vent opens.** ~~Checked and rejected for now: at 30 h the pool
  is about 40 K subcooled, so ε_T = 1 throughout.~~ **CORRECTED the same day:**
  those figures were the fixture from before F10. On the current fixture:
  - **before the vent (to 23.7 h)** the pool is about 53 K subcooled (376 K
    against about 429 K at 0.55 MPa). So ε_T = 1, the model equals MELCOR,
    and the 0–25 h drywell peak is unaffected;
  - **after the vent** the wetwell is at about 0.11 MPa, with about 378 K
    saturation at the vent exit, against a pool at 375–377 K: only 1–3 K
    subcooled. So ε_T ≈ 0.2–0.6, and **the model over-condenses**: MELCOR
    would pass part of the steam into the wetwell gas.

  This matters for the post-vent wetwell and vent flow and for F12 (runs to
  500 h). It is not built yet. **Scoped 2026-10-29:** MELCOR's form is `epsilon = epsilon_z
  epsilon_T`, `epsilon_T = (T_sat(P_junction) - T_pool - 0.1 K) / 5 K`
  clamped to [0, 1] (CVH/FL RM §6, Eqs 6-1 to 6-3, C4405); below 1 the
  steam passes partly to the wetwell atmosphere. Here the wetwell gas has
  no temperature or steam inventory of its own (it is always at the
  pool's temperature and saturation pressure), so any passed steam would
  recondense at once. It needs a wetwell atmosphere first. **Built 2026-10-29**
  (`ww_atmosphere`, `pool_surface.rs`): the wetwell atmosphere has its own
  temperature and steam; the pool surface follows CVH/FL RM §5.1 (Eqs
  5-1 to 5-23) and the bubbles §6.1 (ε_z ε_T, 0.99 relative humidity),
  with MELCOR's NCG/H2O default properties. ~~The torus shell is still not
  a heat structure.~~ The shell above the pool is one since 2026-10-29
  (`ww_shell`, geometry \[A\]).
- **The gap inventory's burst release (RN1_GAP, RN1_GAP00): checked and
  rejected 2026-10-29.** MELCOR's initial gap fraction defaults to zero, and
  the fuel's release held in the gap until 1173 K clad failure is at most
  1.6e-5 of the inventory per hour below 1173 K (CORSOR-M), so it moves
  the release by under 1e-5.
- ~~**F11 can change nothing yet.** No hydrogen reaches the building, because
  the drywell (0.56 MPa) never reaches the flange-leak pressure (0.75 MPa \[A\]).
  The upstream lever is drywell pressure, F8 (MCCI gas). That needs a heat of
  ablation and Unit 1's concrete, neither of which is held.~~ **CORRECTED
  2026-10-29:** neither is a blocker. MELCOR's CAV package ships standard
  concretes (CORCON basaltic is used, \[A\] as Unit 1's) and the ablation
  enthalpy is derived from its composition (1.82 MJ/kg). With the reduced
  CAV (`mcci`) the drywell reaches 0.774 MPa and the flange leak opens at
  about 23 h, so hydrogen now reaches the building (0.04 % by 25 h): F11
  can act.

## 3. Issues

All are sub-issues of #1028, filed 2026-10-28.

| Item | Issue | Priority |
|---|---|---|
| Literature and copyright tiers (§4) | #1031 | P1 |
| F1 comparison harness and ablations | #1032 (closed 2026-10-29) | P1 |
| F2 move into `sembawang` (~~needs go-ahead~~ approved 2026-10-29) | #1033 (closed 2026-10-29) | P2 |
| F3 lower head | #1034 | P1 |
| F4 candling, oxidation, core plate | #1035 | P1 |
| F5 mixture level | #1036 (closed 2026-10-29; fractional coverage left as a refinement) | P2 |
| F6 RPV heat structures, radiation, MSL creep | #1037 | P2 |
| F7 properties from OFFBEAT; reaction heat | #1038 | P2 (`bug` for the 6.45 vs 6.763 MJ/kg disagreement; ~~open~~ resolved 2026-10-29: the derived 6.763 is used) |
| F8 multi-NCG and MCCI gas | #1039 | P2 |
| F9 fission products to `changi` | #1040 | P2 |
| F10 decay heat | #1041 | P3 |
| F11 hydrogen combustion | #1042 | P2 |
| F12 injection bypass, leakage, 500 h | #1043 | P2 |
| F13 input and event-time check | #1044 | P1 |
| F14 Units 2 and 3 (blocked) | #1045 | P3 |

**Status of the items, 2026-10-30** (maintainer goal: "finish everything
except the control volumes"). Checked against the rows above and the code:

- **The MELCOR package models outside CVH/FL** are now in, each default on
  with an exact ablation, or else checked against the manual and found to be
  handled as upstream's defaults handle them. That covers B4C, BUR burn
  dynamics, FDI LPME, COR_CMT UO2 transport and the lower-plenum molten pool;
  the checked-only ones are HPME/DCH, eutectics, cavity layers and in-core
  pools (rows in §2a).
- **F12's injection bypass** is not a MELCOR model. It is a scenario input,
  the fraction of injected water that misses the core, and has no public
  source held. The BSAF full report (L9) is not held, and TEPCO data (L8)
  awaits a data-policy ruling. Picking a value to match the measured
  pressure would be calibration, so it waits on literature. F12's leak paths
  and the 500 h run are in.
- **F13** (the input and event-time check) is blocked on L4-L9 (§4).
- **F14** (Units 2 and 3) needs the control-volume equation of state, which
  is excluded from this goal: the thermal hydraulics are to be partitioned
  into `tampines` and `outram-foam`.

## 4. Literature needed, and its copyright

**The tiers below are expected, not verified.** A document's tier comes from
its own copyright statement (`DATA_POLICY.md`; `crates/kovan-literature/CLAUDE.md`),
and none of these could be opened here: this environment's network policy
blocks `nrc.gov`, `osti.gov` and `sandia.gov`. Each is checked against its
title page on ingestion, and the tier corrected if it differs.

Tier key:

- **public**: may go into `reactor-literature` (public), with a verified
  redistribution basis recorded in its README row.
- **private**: copyrighted with no redistribution grant. The PDF goes to the
  private repository; facts and numbers are cited with acknowledgement.
- **ruling**: needs a maintainer decision before anything is done with it.

Identifiers marked † are from memory and must be confirmed on retrieval.

| Code | Document | Used by | Expected tier and basis |
|---|---|---|---|
| **L1** | MELCOR Computer Code Manuals Vol. 2, Reference Manual, SAND2017-0876 O (ADAMS ML17040A420) | F3–F9 | ~~**public**: Sandia report for NRC, unlimited release; NRC ADAMS public~~ **private**, verified on ingestion 2026-10-29: no release or licence statement (`CATALOGUE.md` `sandia2023melcorvol2`). Held edition: **SAND2023-10994 O, 2.2 r2023.0** (ML23116A100), not SAND2017-0876 O |
| **L2** | MELCOR Vol. 1, Primer and Users' Guide, SAND2017-0455 O (ML17040A429); later rev. SAND2021-0726 O (ML21042B319) | F5, executive | ~~**public**: as L1~~ **private**, verified 2026-10-29 (`sandia2017melcorvol1`); SAND2017-0455 O held |
| **L3** | MELCOR 1.8.5 Manuals Vol. 2 Rev. 2, NUREG/CR-6119 / SAND2000-2417 (ML010190117), incl. the COR and BH package references (ML010190222) | F3, F4, F6 | **public**: NUREG/CR, US government |
| **L4** | Gauntt et al., *Fukushima Daiichi Accident Study (Status as of April 2012)*, SAND2012-6173 | F6, F13, F14 | **public**: Sandia, unlimited release (to verify) |
| **L5** | INPO 11-005, *Special Report on the Nuclear Accident at the Fukushima Daiichi NPS* (2011) | F13 | **private**: © INPO |
| **L6** | IAEA, *The Fukushima Daiichi Accident*, Technical Volume 1 (2015) | F13 | **private**: © IAEA |
| **L7** | Government of Japan, *Report to the IAEA Ministerial Conference on Nuclear Safety* (June 2011), attachments | F13 | **private** until its statement is checked |
| **L8** | TEPCO accident investigation reports and plant parameter data | F13, F12 | **ruling**: © TEPCO, *and* measured plant data sits close to `DATA_POLICY.md`'s "operational facility data". Until ruled on, the BSAF-published traces (held) stand in |
| **L9** | OECD/NEA BSAF Phase II full report (the summary, NEA No. 7525, is already held, private); Phase I report | F12, F13, F14 | **private**: © OECD |
| **L10** | Rempe et al., *Light Water Reactor Lower Head Failure Analysis*, NUREG/CR-5642† (1993); Chu et al., *Lower Head Failure Experiments and Analyses*, NUREG/CR-5582† (1998/99), incl. SA533B creep data | F3, F6 | **public**: NUREG/CR, US government |
| **L11** | OECD/NEA OLHF programme report (lower-head failure) | F3 (optional) | **private**: © OECD |
| **L12** | CORA (Hagen et al., KfK reports) and QUENCH (FZKA/KIT reports) bundle tests | F4 V&V | **private** until KIT's statement is checked |
| **L13** | Urbanic & Heidrick, *J. Nucl. Mater.* 75 (1978); Cathcart et al., ORNL/NUREG-17 (1977); Baker & Just, ANL-6548 (1962) | F4, F7 | Urbanic–Heidrick **private** (© Elsevier); ORNL/NUREG-17 and ANL-6548 **public** (US government) |
| **L14** | SCDAP/RELAP5 MATPRO, NUREG/CR-6150† Vol. 4 (the manual only, never the code); Fink, *J. Nucl. Mater.* 279 (2000) | F7 | MATPRO **public** (NUREG/CR); Fink **private** (© Elsevier). Check what OFFBEAT already cites first |
| **L15** | CORCON-Mod3 manual, NUREG/CR-5843†; OECD MCCI / CCI final reports; Unit 1 concrete composition (source to find) | F8 | CORCON **public**; OECD **private**; concrete: unknown |
| **L16** | NUREG-0772 (1981); CORSOR-M, NUREG/CR-4173†; MAEROS, NUREG/CR-1391†; SPARC-90, NUREG/CR-5765†; NUREG-1465; PHÉBUS-FP FPT-1 final report | F9 | NUREGs **public**; PHÉBUS **private** (© IRSN/EC) |
| **L17** | A Unit 1 core inventory: Nishihara et al., JAEA-Data/Code 2012-018† — or computed with `outram-mc` depletion, which needs no literature beyond the core design (L4) | F9, F10 | JAEA **private** until its statement is checked |
| **L18** | ANS-5.1 decay heat standard (2014 edition) | F10 | **private**: a purchased standard (`melcor-parity-scoping.md` §1.7); only if the maintainer holds a copy. The 1978 draft already in `teh-o-prke` covers the actinide cases |
| **L19** | Coward & Jones, US Bureau of Mines Bulletin 503 (1952), flammability limits; NEA/CSNI/R(2000)7, *Flame Acceleration and DDT in Nuclear Safety* | F11 | Bulletin 503 **public** (US government); CSNI **private** (© OECD) |
| **L20** | A public drift-flux model for the mixture level: an NRC code's theory manual (manual only); Zuber & Findlay, *J. Heat Transfer* 87 (1965) | F5 | NRC manual **public**; Zuber–Findlay **private** (© ASME) |
| **L21** | Radiation between core components: L1/L3's COR sections; a heat-transfer textbook (e.g. Siegel & Howell) | F6 | L1/L3 **public**; textbook **private** (maintainer's own copy) |
| **L22** | Sandia reports on RCIC (Terry turbine) beyond-design operation† | F14 | **public** (to verify) |

**Most needed first:** L1–L4 (public, but blocked by the network policy), L9
(full BSAF report), L10, L13 and L16. Then L5–L7 for F13.

## 5. What this plan does not do

- No MELCOR, ASTEC, MAAP, RELAP5, SCDAP/RELAP5 or MOOSE-application source, even
  "just to check an equation" (`melcor-parity-scoping.md` §1.1, §8).
- No calibration toward BSAF bands, and nothing gated on Tier C or D quantities.
- No plant data that `DATA_POLICY.md` excludes. L8 waits for a ruling.
