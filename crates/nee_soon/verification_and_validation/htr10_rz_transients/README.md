# HTR-10 LOFC and CRW ATWS with the R-Z porous core (gh:#1151 part B)

> Research, education and V&V only. Not for licensing, operation or safety
> decisions. AI draft (Claude), unreviewed by a human.

Code: `crates/outram-foam-appbuilder-lib/src/htr10_rz/transient.rs` (the
transient R-Z core), `crates/tampines/src/htr10_primary_loop/rz_core.rs`
(its loop interface, TH only: the power is an input), and in
`crates/nee_soon/src/htr10_th_transient/` `transient.rs` (the cases), `kinetics.rs` (the
neutron source and the external reactivity). Inputs: the cited input sheet
`docs/reactor-scoping/htr10-th-benchmark.md` and `htr10-th-data/`; source
keys ([T1] TECDOC-1694, [C09] Chen 2009, [H06] Hu 2006, [J09] Jun 2009,
[G02] Gao & Shi 2002, [C14] Chen 2014, [S21] She 2021) are the sheet's.

## 1. Predictions, written before the first coupled run (2026-11-03)

Written after the R-Z transient code was written and before any coupled
LOFC or CRW run, from hand estimates. Committed in its own commit before
any result.

**Model.** The gh:#1149/#1150 R-Z solid field marched by backward Euler; the
porous gas network solved quasi-steadily each core step (THERMIX's own
coupling, [C09] Sec 3.3) with buoyancy in the bed, the discharge tube, the
bottom-reflector borings and the control-rod channels (dropped in the open
plena, as in gh:#1150); a fuel-zone node per heated bed cell; Tab 2.56 (LOFC)
or Tab 2.61 (CRW) power shape; RCCS at 50 degC. Point kinetics as part A,
plus the Am-Be start-up source (4.4e7 n/s, TECDOC-1382 Sec 4.2.1.5;
presence at the test Unknown) ON by default.

**Hand estimates.**

- Bed heat capacity: 5.0 m^3 x 0.61 x ~2.4 MJ/(m^3 K) ~ 7.3 MJ/K (about the
  lumped stand-in's 8.6 MJ/K; the stand-in counted the cone pebbles
  separately). Short-term heat-up after the baffle closes ~0.4 K/s:
  the short-term power fall should be close to part A's (257 s to 1 %).
- In-vessel natural circulation in the bed, Darcy with KTA 3102.3 at
  2.5 MPa: `dp/dz ~ 6 G + 130 G^2` Pa/m against a buoyancy of ~1-5 Pa/m for
  a 50-200 K radial difference, so `G ~ 0.05-0.3 kg/(m^2 s)` and an upflow
  of **0.02-0.3 kg/s** in the hot centre, carrying 10-100 kW to the top of
  the bed and the core-entry plenum per 100 K of difference.
- Measured top-reflector heat-up (+170 K in 3 h, [C09]) on ~13 t of
  graphite is ~250 kW average; the model will get some of it through the
  entry plenum (a conduction-plus-radiation medium) and the circulation.
  Bed heat loss after the trip: side reflector 40-60 kW (part A) plus top
  100-200 kW from the circulation: **2-4 times part A's**.
- Net cooling of the bed ~100-200 kW against ~60 kW decay heat at 1000 s:
  the -500 pcm minimum plus ~-150 pcm xenon need ~35 K of cooling, i.e.
  1500-3500 s after the minimum.
- Neutron source floor: the subcritical fission power is held at
  `S/(nu F0 |rho|)` ~ 1.8e-10 / 0.005 ~ 4e-8 of the initial instead of
  falling for hours (part A's ~1e-20). Return from the floor: the time to
  climb `ln(0.01/4e-8) ~ 12` e-folds instead of ~40 shortens the
  overshoot; predicted sign: **earlier return, lower first peak** than a
  source-free run.

### LOFC (RZ-BASE)

| Quantity | Prediction | Measured / reference |
|---|---|---|
| Fission power to 1 % | 250-350 s | ~330 s ([C09]) |
| Minimum total reactivity | -400 to -650 pcm at 400-1000 s | THERMIX ~-400 pcm (calc.) |
| Re-criticality (rho = 0) | 2500-5000 s | 2520 s [H06] / 4075 s [T1] / THERMIX 3090 s |
| Power back above 1 % | 3000-6000 s | ~4210 s (digitised [C09] Fig 6) |
| First peak | 15-50 % | 24.6 % at 4395 s |
| Mean power, last 30 min | 120-250 kW | ~180 kW |
| Bed upflow after the trip | 0.02-0.3 kg/s | not measured |
| TR1&TR4 rise in 3 h | +80 to +250 K | +173 K |
| SR1-SR12 | within +-50 K of measured, inner points high | THERMIX within 18-51 K |
| NC-OFF ablation | later re-criticality than RZ-BASE (by >1000 s) | - |
| SRC-OFF ablation | later return, higher first peak | - |

### CRW (CRW-BASE, trip at 12 s)

| Quantity | Prediction | Measured |
|---|---|---|
| First peak | 150-300 % at 25-45 s | 216 % at 35 s ([H06]) or 241 % at 30 s ([J09]) (N7) |
| Power to decay level | 500-1200 s | 810 s ([H06]) |
| Re-criticality | 2000-5000 s | 2400 s [H06] / ~3400 s [T1] |
| Re-criticality peak | 15-50 % | 27 % at 3420 s [J09]; 0.28 at ~3400 s [T1] |
| Long-term power | 120-250 kW | ~200 kW at ~7200 s ([H06]) |
| Top reflector rise in 2 h | +80 to +250 K | +215 K ([H06]); +212 K ([J09]) |
| CRW-TRIP9 | lower first peak than CRW-BASE | - |

**The too-fast short-term power fall of part A** (0.147 vs 0.204 at 100 s).
Hypotheses, each with its predicted sign before measuring: (a) the fuel
zone's own capacity (lag ~7 s) delays the fuel heat-up: sign right (slower
fall), magnitude small (fuel coefficient only -2.13 pcm/K); (b) the
reflector heats in the first minutes through the 12 s of forced flow and
then through the gas: positive reactivity, slower fall, size unknown; (c)
the bed heat capacity of the R-Z model (whole 5 m^3 at 0.61) is a few %
above the stand-in's: slower fall by a few %; (d) the measured trace is an
ex-core signal whose relation to core fission power during a fast
temperature change is unknown (not testable here). The neutron source
cannot matter above 1e-7 of the power.

*(Sec 1 was committed in `f3e6f8b18`, before any coupled R-Z LOFC or CRW
run. Everything below was written after the runs.)*

## 2. Methodology

**What is computed.** The two HTR-10 ATWS tests (input sheet Sec 7, 8) with
the R-Z core in the loop:

- **LOFC** (`LofcCase::rz_base`, `RZ-BASE`): as-run 3315 kW / 212 degC /
  2476 kPa / 1.457 kg/s (D4); measured C09 coastdown; baffle and isolation
  at 12 s; Tab 2.56 shape; 3 h.
- **CRW** (`LofcCase::crw_base`, `CRW-BASE`): same initial state; Tab 2.61
  shape; Tab 2.60 reactivity added to the feedback (held at 4.9762 mk after
  128 s); trip, baffle and isolation at 12 s (D4, [H06]); `CRW-TRIP9`: flow
  stop and isolation instantly at 9 s ([T1] spec). 3 h.

**Model.** Section 1's description; the parts and their data:

| Part | Where | What |
|---|---|---|
| R-Z core in time | `outram-foam-appbuilder-lib::htr10_rz::transient` | backward-Euler solid on the gh:#1149 field (20 cm cells, 22 x 66 = 1452 cells, the Fig 2.5 lines kept); quasi-steady gas on the gh:#1150 network each core step, explicit in the gas as THERMIX ([C09] Sec 3.3); buoyancy ON in the bed, tube, borings and CR channels; gas `rho c` dropped (~30 kJ/K against ~8 MJ/K of bed) |
| Fuel zones | same | one node per heated cell, capacity `x_fuel (2.5/3)^3 = 0.33` of the bed `rho c` (the cell keeps 0.67), `G = 1/(R_zone + R_shell)` per fuel pebble with Tab 2.9 `k`; eliminated exactly (`h' = G a/(a+G)` to `T_f^n + q/a`), enthalpy-form update |
| Feedback temperatures | same | fuel: power-weighted fuel nodes; moderator: power-weighted bed cells; reflector: volume mean of all graphite reflector cells ([T1] Sec 2.1.1.6 defines each coefficient for a uniform change; the weighting is a named assumption, `REFL-SIDE` the part A alternative) |
| Loop interface | `tampines::htr10_primary_loop::RzPorousCore` | sub-cycles the R-Z core at 1 s (to 1000 s) / 5 s with the time-mean power; `K W|W|` and gravity head frozen at the steady state; heat source is an input |
| Kinetics | `nee_soon::htr10_th_transient::kinetics` | part A's, plus the Am-Be source and the external reactivity; the initial state is held exactly steady with the source by a -1.8e-10 compensation (`source_compensation`) |

**References and instruments.** Power: the digitised measured [C09]
Fig 4 / Fig 6 traces (part A, Sec 4). Thermocouples: [C09] Figs 8-11, 17
traces in 180 s bins; model positions in
`c09_thermocouple_positions` (Uncertain, fixed before the runs: SR1-12 at
[T1] Tab 2.7's TC 1-12, TR1&TR4 at TC 13, TR2&TR5 / TR3&TR6 read from
[C09] Fig 3 at r 55, z -100 / -135, BR pairs at r 78 / 45, z 234), read by
the gh:#1150 instrument rule (gas in open volumes, solid elsewhere). CRW:
the quoted numbers of input sheet Sec 8.3 (no trace digitised: the CRW
figures were not digitised in this change).

**She 2021 Fig 8 does not apply.** Its markers are core-outlet temperatures
over steady operating periods (days 480-1680) at the PANGU input power,
with their own inlet temperatures and flows; no LOFC or CRW. Not compared.

**Gate.** Only part A's, fixed before any run: LOFC time to 1 % within
330 +- 120 s (`rz_lofc_base_against_the_measured_traces`). Everything
else is reported, not gated.

**Numerical checks.** Core time step halved (`DT-HALF`: 0.5 / 2.5 s); mesh
15 / 20 / 30 cm (`rz_lofc_mesh_check`); energy conservation and the
buoyancy ablation in the appbuilder unit tests.

## 3. Results (2026-11-03, `cargo test --release -p nee_soon --lib htr10_th_transient`)

### 3.1 Initial state and natural circulation (TH only, power as input)

| Quantity | R-Z (20 cm) | Lumped (part A) | Measured |
|---|---|---|---|
| Core outlet | 636.19 degC | 637.96 degC | 650 degC (flow derived assuming zero loss) |
| Heat to RCCS + heads | 104.3 kW | 90.9 kW | Unknown |
| Power-weighted fuel / moderator / reflector | 580.9 / 574.3 / 310.8 degC | 546.1 / 508.6 / 214.8 | - |

The R-Z reflector temperature is a different quantity (all graphite, not
part A's side node), so the reflector reference differs by design.

**Natural circulation** (`htr10_rz_transient_images`, loop stopped, a fixed
150 kW for 600 s): bed upflow across mid-height **0.063 kg/s** (and the same
downflow: mass balanced), largest bed mass flux 0.11 kg/(m^2 s). In RZ-BASE
the upflow is 0.04 kg/s at 20 s, peaks at **0.093 kg/s** near 3000 s and is
0.057 kg/s at 3 h: inside the predicted 0.02-0.3 kg/s.
`rz_transient_natural_circulation.png` shows where it runs: the bed, the
core-entry plenum, the top-reflector flow zone, the bottom-reflector
borings, the hot plenum and the discharge tube. The control-rod channels
carry ~1e-5 kg/(m^2 s): closed at the top (the chain carries no flow), they
are dead ends. **What is not represented:** buoyancy in the plena (kept
off, gh:#1150) and the riser loop (bed up, top plenum, risers down, bottom
cavity, discharge tube up), which in this network needs the inlet chain to
carry flow. Hand bound: ~40 Pa of head over ~5 m against the discharge
throttle (void 0.0067, `D_h` 1 cm assumed); its size is Unknown here.

### 3.2 LOFC ATWS: prediction against result

| Quantity | Predicted (Sec 1) | RZ-BASE | Lumped (part A, source-free) | Measured |
|---|---|---|---|---|
| Fission power to 1 % | 250-350 s | **288 s** (gate: passes) | 257 s | ~330 s |
| Minimum total reactivity | -400 to -650 pcm at 400-1000 s | **-370 pcm at 3080 s** (miss: shallower; a plateau -345 pcm from 300 s, deepened later by xenon) | -547 pcm at 942 s | THERMIX ~-400 pcm |
| Re-criticality (rho = 0) | 2500-5000 s | **7616 s** (miss, late) | none in 3 h | 2520 / 4075 s; THERMIX 3090 s |
| Power back above 1 % | 3000-6000 s | **8516 s** (miss, ~4300 s late) | none | ~4210 s |
| First peak | 15-50 % | **22.4 % at 8705 s** (magnitude inside, time late) | none | 24.6 % at 4395 s |
| Mean power, last 30 min | 120-250 kW | **109 kW** (miss, low) | 32 kW | ~180 kW |
| Bed upflow after the trip | 0.02-0.3 kg/s | 0.04-0.093 kg/s | 0 | not measured |
| TR1&TR4 rise in 3 h | +80 to +250 K | **+219.5 K** (274.8 -> 494.3 degC) | - | +172.5 K (232.7 -> 405.1); THERMIX +239 K |
| SR1-SR12 | within +-50 K | **all within 46 K**; RMS per TC 4-29 K | - | THERMIX within 18-51 K |
| NC-OFF | re-criticality later by > 1000 s | **earlier by 2115 s** (5501 s) | - | - |
| SRC-OFF | later return, higher first peak | **later by 1127 s, 160 % peak** | - | - |

Relative fission power, short term (model / measured): 10 s 0.984 / 0.989;
20 s 0.855 / 0.905; 50 s 0.433 / 0.496; 100 s 0.162 / 0.204; 200 s 0.033 /
0.047; 300 s 0.0085 / 0.0139; 400 s 0.0025 / 0.0056. **Still faster than
the plant**, less so than part A (0.147 at 100 s). Plots:
`rz_lofc_power_short.png`, `rz_lofc_power_long.png`, `rz_lofc_power_log.png`
(the source floor, ~5e-8 of the initial fission power from 2000 s, as
predicted: `S / (nu F0 |rho|)` = 1.8e-10 / 0.0035), `rz_lofc_reactivity.png`.

**Thermocouples** (`rz_lofc_thermocouples_*.png`, every 180 s bin, 3 h):
RMS over all 17 traces and bins **32.4 K**, mean +9.0 K.

| TC | r, z cm | measured t0 / 3 h | model t0 / 3 h | residual t0 / 3 h K | RMS K | max abs K |
|---|---|---|---|---|---|---|
| SR1 | 193, 80 | 219.2 / 216.3 | 220.9 / 227.4 | +1.6 / +11.1 | 7.0 | 11.2 |
| SR2 | 189, 80 | 244.2 / 250.0 | 250.0 / 260.5 | +5.9 / +10.5 | 7.2 | 10.4 |
| SR3 | 167, 80 | 264.8 / 279.7 | 282.2 / 310.4 | +17.5 / +30.7 | 28.2 | 30.7 |
| SR4 | 133, 80 | 278.8 / 297.9 | 299.4 / 321.0 | +20.6 / +23.2 | 24.5 | 26.2 |
| SR5 | 117, 80 | 293.2 / 307.9 | 320.6 / 330.5 | +27.4 / +22.5 | 27.0 | 30.4 |
| SR6 | 93, 80 | 324.3 / 342.0 | 365.4 / 354.5 | +41.1 / +12.5 | 29.3 | 45.6 |
| SR7 | 193, 170 | 222.4 / 224.2 | 220.4 / 233.6 | -2.0 / +9.4 | 4.4 | 9.6 |
| SR8 | 189, 170 | 267.9 / 279.8 | 249.2 / 270.4 | -18.8 / -9.4 | 16.3 | 20.1 |
| SR9 | 167, 170 | 314.6 / 329.9 | 281.0 / 329.9 | -33.7 / -0.0 | 11.7 | 33.0 |
| SR10 | 133, 170 | 341.7 / 351.4 | 310.7 / 340.3 | -30.9 / -11.1 | 16.6 | 30.1 |
| SR11 | 117, 170 | 371.0 / 361.0 | 349.7 / 348.1 | -21.4 / -13.0 | 14.4 | 21.3 |
| SR12 | 93, 170 | 462.7 / 387.0 | 431.0 / 362.2 | -31.7 / -24.8 | 22.8 | 32.2 |
| TR1_TR4 | 60, -40 | 232.7 / 405.1 | 274.8 / 494.3 | +42.2 / +89.1 | 103.1 | 114.5 |
| TR2_TR5 | 55, -100 | 218.5 / 269.4 | 257.6 / 261.1 | +39.1 / -8.3 | 24.6 | 39.1 |
| TR3_TR6 | 55, -135 | 202.7 / 219.6 | 234.6 / 235.2 | +31.9 / +15.6 | 25.8 | 31.9 |
| BR1_BR3 | 78, 234 | 642.8 / 431.1 | 655.2 / 394.1 | +12.4 / -37.0 | 37.7 | 48.0 |
| BR2_BR4 | 45, 234 | 748.0 / 444.1 | 728.5 / 426.1 | -19.4 / -18.1 | 7.8 | 24.0 |

SR6's minimum: model 334.8 degC at ~3780 s, measured 295 degC at 2800 s
(THERMIX 335 degC at 4200 s). The TR1&TR4 point sits on the face between
the top-reflector flow zone and the core-entry plenum, so the instrument
averages solid and plenum gas: it jumps by ~65 K in the first 180 s as hot
gas reaches the plenum. Read it as Mixed; the measured thermocouple is in
graphite.

**Reading it.** The in-vessel heat paths are now in the model and the
temperatures show it: the top reflector heats by +220 K (measured +173,
THERMIX +239), and with `NC-OFF` it would not (CRW-NC-OFF: +9 K). The side
reflector is within 46 K everywhere. **The re-criticality is ~3400 s late
and the long-term power 40 % low**, for one reason the numbers point at:
the model's **ultimate heat sink is ~100 kW** (RCCS + heads, 97-104 kW all
run) while the plant's long-term power, which equals its heat loss once
steady, is ~180 kW. With ~80 kW less removed, the bed's power-weighted
temperature falls only 24 K in 2 h (594 -> 570 degC). And the circulation
works against an early return: it carries heat from the hot lower core to
the cold upper core, where the power weight is largest, so `NC-OFF` returns
**earlier** (5501 s), against the prediction. The prediction's sign was wrong
because it reasoned only about heat leaving the bed, not about where in the
bed the heat goes.

### 3.3 LOFC sensitivity and ablation cases (one change from RZ-BASE each)

| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min | max bed upflow after 1000 s |
|---|---|---|---|---|---|---|---|
| RZ-BASE | 288 s | -370 pcm @ 3080 s | 7616 s | 8516 s | 22.4 % @ 8705 s | 109 kW | 0.093 kg/s |
| NC-OFF (no buoyancy) | 292 s | -338 pcm @ 228 s | 5501 s | 7200 s | 5.9 % @ 7500 s | 51 kW | 0 |
| SRC-OFF (no source) | 288 s | -370 pcm @ 3080 s | 7616 s | 9643 s | 160.4 % @ 9705 s | 219 kW | 0.093 kg/s |
| SRC-INTRINSIC (775 n/s) | 288 s | -370 pcm @ 3080 s | 7616 s | 8867 s | 43.6 % @ 8995 s | 110 kW | 0.093 kg/s |
| FUEL-QS (no fuel-zone storage) | 297 s | -357 pcm @ 3105 s | 7551 s | 8454 s | 20.8 % @ 8645 s | 108 kW | 0.093 kg/s |
| REFL-SIDE (part A's reflector) | 291 s | -341 pcm @ 243 s | 6841 s | 7756 s | 22.0 % @ 7950 s | 119 kW | 0.093 kg/s |
| BED-ZBS (D3 comparison) | 294 s | -334 pcm @ 230 s | 5336 s | 6253 s | 22.7 % @ 6445 s | 164 kW | 0.092 kg/s |
| BYP-G02 | 288 s | -364 pcm @ 3025 s | 7516 s | 8411 s | 22.6 % @ 8600 s | 111 kW | 0.092 kg/s |
| BYP-NOLEAK | 284 s | -421 pcm @ 3375 s | 8431 s | 9371 s | 19.8 % @ 9570 s | 120 kW | 0.095 kg/s |
| TR-SPEC (as-run vs spec) | 298 s | -332 pcm @ 2950 s | 7606 s | 8552 s | 22.2 % @ 8755 s | 97 kW | 0.088 kg/s |
| KIN-ALPHA-G02-EQ | 380 s | -264 pcm @ 3525 s | 9541 s | none | none | 32 kW | 0.091 kg/s |
| XE-OFF | 290 s | -343 pcm @ 247 s | 6556 s | 7466 s | 22.4 % @ 7655 s | 145 kW | 0.093 kg/s |
| DT-HALF (core step 0.5 / 2.5 s) | 288 s | -370 pcm @ 3060 s | 7561 s | 8457 s | 21.9 % @ 8645 s | 109 kW | 0.093 kg/s |
| MESH-30CM (1210 cells) | 288 s | -370 pcm @ 3075 s | 7616 s | 8514 s | 22.4 % @ 8705 s | 109 kW | 0.093 kg/s |
| MESH-15CM (2136 cells) | 287 s | -408 pcm @ 3350 s | 7876 s | 8744 s | 23.6 % @ 8930 s | 122 kW | 0.098 kg/s |
| MESH-10CM | **failed**: the gas-flow Picard stalls at a relative change 7.0e-7 against its 1e-7 tolerance (also with 1000 passes instead of 200, tried and reverted) | | | | | | |

Reading it:

- **Which numbers to quote:** RZ-BASE. The numerical uncertainty is
  ~+-60 s on the re-criticality from the time step and ~+260 s from 20 to
  15 cm; neither touches the ~3400 s miss. The 10 cm mesh does not run
  (unresolved; the gas Picard's convergence measure has a floor there).
- **The bed conductivity is the strongest physical lever** (ZBS: -2280 s,
  164 kW late power), then xenon (-1060 s), then the reflector weighting
  (-775 s). None is a free parameter and none is picked because it is
  closer; ZBS is the D3 comparison, not the base.
- **The source matters only for the return's shape**: same re-criticality,
  but a 160 % overshoot without it and 22 % with it (measured 24.6 %). It is
  the floor, not the time, that the source sets.
- **The fuel-zone lag is not the short-term cause**: removing it makes the
  1 % time 9 s *slower* (the lag holds fuel heat away from the moderator
  coefficient for a few seconds).

### 3.4 The too-fast short-term fall (item 4)

Predictions (Sec 1) against the measurements:

| Hypothesis | Predicted sign | Measured effect at 100 s / on the 1 % time | Verdict |
|---|---|---|---|
| (a) fuel-zone storage | slower fall, small | FUEL-QS 297 vs 288 s: the lag *speeds* the fall slightly | not the cause |
| (b) reflector heating early | slower fall | reflector feedback +0.2 K in 300 s (+1.5 pcm); REFL-SIDE 291 s | not the cause |
| (c) R-Z bed capacity | slower fall, few % | 0.162 vs the stand-in's 0.147 at 100 s (1 % at 288 vs 257 s) | right sign, explains ~25 % of the gap |
| (d) the measured signal is ex-core | unknown | not testable here | open |

The gap at 100 s is still 0.162 against 0.204. The coefficient set moves
the 1 % time most (G02-EQ: 380 s), but the base set stays the [T1] /
[J09] one. **New hypothesis, not tested** (written after the runs): the
homogeneous bed gives fuel-pebble shells and dummy pebbles one
temperature; in the plant the dummies (43 % of the mixed zone) heat only
through contact, radiation and gas, so the moderator term lags the fuel.
Predicted sign: slower fall and a later, higher CRW peak. Magnitude
Unknown; it needs a two-temperature bed.

### 3.5 Neutron source (item 4)

TECDOC-1382 Sec 4.2.1.5: 20 Ci Am-Be, 4.4e7 n/s, in the side reflector for
the start-up; whether it was in on 15 Oct 2003 is **Unknown**. Default ON
(`NeutronSource::htr10_am_be_startup`, pinned by
`neutron_source_is_default_and_sets_the_subcritical_floor`), importance 1
(an upper bound for a reflector source), ablations `SRC-OFF` and
`SRC-INTRINSIC` (U-238 SF, 775 n/s, a lower bound). Verified: at a held
-500 pcm the fission power settles to `Q_f w S / (nu |rho|)` to < 1e-3. **It
changes the part A record** (default changed: re-measured, part A README
Sec 3.4 correction): BASE-6H returns at 12406 s with an 8.7 % peak instead
of 15038 s and 80.8 %; XE-OFF now returns in 3 h (9652 s); the short-term
metrics are unchanged.

### 3.6 CRW ATWS

| case | first peak | to decay level | min rho | re-criticality | re-criticality peak | P at 7200 s | mean P, last 30 min | TR1&TR4 rise at 2 h |
|---|---|---|---|---|---|---|---|---|
| CRW-BASE | 158.4 % @ 23 s (5162 kW) | 272 s | -308 pcm @ 303 s | 6041 s | 21.9 % @ 7170 s | 694 kW | 135 kW | +222 K |
| CRW-TRIP9 | 145.1 % @ 22 s (4741 kW) | 275 s | -299 pcm @ 307 s | 5996 s | 21.7 % @ 7135 s | 563 kW | 132 kW | +222 K |
| CRW-SRC-OFF | 158.4 % @ 23 s | 272 s | -308 pcm @ 303 s | 6041 s | 111.8 % @ 7955 s | 35 kW | 138 kW | +218 K |
| CRW-NC-OFF | 158.5 % @ 23 s | 274 s | -299 pcm @ 277 s | 3256 s | 9.1 % @ 4885 s | 41 kW | 74 kW | +9 K |

| Quantity | Predicted | CRW-BASE | Measured |
|---|---|---|---|
| First peak | 150-300 % at 25-45 s | **158 % at 23 s** (5162 kW) | 216 % at 35 s ([H06], per 3315 kW) / 241 % at 30 s ([J09], = 7230/3000); 7230 kW absolute |
| ... against both normalisations (N7) | - | **-58 points vs 216 %; -83 points vs 241 %**; on a 3000 kW base the model reads 175 % | - |
| Power to decay level | 500-1200 s | **272 s** (miss, early) | 810 s |
| Re-criticality | 2000-5000 s | **6041 s** (miss, late) | 2400 s [H06] / ~3400 s [T1] |
| Re-criticality peak | 15-50 % | **21.9 % at 7170 s** (magnitude inside, time late) | 27 % at 3420 s [J09] |
| Long-term power | 120-250 kW | **135 kW** (inside, low) | ~200 kW |
| Top-reflector rise in 2 h | +80 to +250 K | **+222 K** | +215 K [H06], +212 K [J09] |
| CRW-TRIP9 | lower first peak | 145 % (lower, as predicted) | - |

The first peak is low and early whichever normalisation is used, and the
model is shut down by feedback too quickly afterwards (decay level at
272 s against 810 s): the same direction as the LOFC's too-fast fall, which
the untested dummy-pebble lag (3.4) would move the right way. Re-criticality
late and long-term power low: the heat-sink deficit of 3.2. The top
reflector rise matches. Plots: `rz_crw_power_short.png`,
`rz_crw_power_long.png`; trace `rz_crw_base_trace.csv`.

## 4. Unresolved, stated

1. **Ultimate heat sink ~80 kW short** (model ~100 kW, plant ~180 kW at
   3 h). Leading hypothesis, predicted sign and size before any test:
   **natural convection of air in the RPV cavity**, not modelled
   (`materials.rs`, `model.rs` say so): `h ~ 3-4 W/(m^2 K)` over the ~150
   m^2 RPV side at a ~100-150 K difference is **45-90 kW**, more removal ->
   earlier re-criticality and a higher plateau. Needs a combined
   radiation + convection boundary (the field takes one condition per
   patch). Not tuned, not tested.
2. Short-term fall and CRW peak (3.4): dummy-pebble lag hypothesis untested.
3. Riser loop and plena buoyancy not represented (3.1).
4. The 10 cm mesh's gas Picard does not converge (3.3).
5. Thermocouple positions are read from a schematic (Uncertain); BR pairing
   chosen from the initial readings.
6. The Am-Be source's presence at the test is Unknown; the default uses it.
7. Loop resistance and gravity head frozen at the steady state (the loop
   flows 12 s); reversed loop flow refused by the R-Z core.
8. Part A's flaws 4-9 (bypass split, group constants, Xe one-group, loop
   data, Tab 2.8 helium) carry over.

## 5. Drawings (geometry rule)

- `rz_transient_mesh_flow_regions.png`: the 20 cm transient mesh and its
  flow regions from the assembled mesh and network (`cell_at` per pixel,
  `cell_flow` per cell). Checked: the stepped cone (6 x 7.5 cm), the
  discharge tube to z 484 and its throttle to 507, the CR channels -65 to
  222, the risers r 140.6-148.6 to z 446, the hot plenum r 25-90 at z
  295-325, the leak zone r 39-50 from 325 to 446, the annulus and the inlet
  path at r 197-210: the same regions as the 5 cm FPIC drawing
  (`crates/outram-foam-appbuilder-lib/verification_and_validation/htr10_rz_flow_fpic/`).
  Not checkable at this size: cells under 5 cm (the 4 cm brick gap, 3 cm
  barrel) are visible only as line pairs.
- `rz_transient_solid_t_steady.png`, `rz_transient_solid_t_stopped.png`,
  `rz_transient_natural_circulation.png`: the as-run steady temperatures, and
  after 600 s with the loop stopped at 150 kW, with the mass flux.
- `coupled_model_topology.svg`: what the kinetics, the R-Z core and the loop
  exchange (topology only; hand-written, not rendered here: no SVG
  rasteriser in this environment).

## 6. How to run

```bash
cargo test --release -p nee_soon --lib htr10_th_transient -- --nocapture          # all; long ones behind long-tests (default on)
cargo test --release -p outram-foam-appbuilder-lib --test htr10_rz_transient_images  # drawings
cargo test --release -p outram-foam-appbuilder-lib --lib transient_tests            # unit checks
```

The runs also write the traces as CSV (`rz_*_trace.csv`,
`rz_lofc_base_thermocouples.csv`) into this folder; `*.csv` is gitignored
under `verification_and_validation/`, so they are regenerated, not
committed. Wall time (4 cores): RZ-BASE 58 s; the sensitivity set ~10 min; CRW set
~3 min; mesh check ~2 min.
