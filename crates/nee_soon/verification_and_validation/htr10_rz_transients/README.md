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
~~15 / 20 / 30 cm~~ **30 / 20 / 15 / 10 cm** (`rz_lofc_mesh_check`; the
10 cm mesh since gh:#1175, Sec 3.3.1); energy conservation and the
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
| ~~MESH-10CM~~ | ~~**failed**: the gas-flow Picard stalls at a relative change 7.0e-7 against its 1e-7 tolerance (also with 1000 passes instead of 200, tried and reverted)~~ **CORRECTED 2026-11-03 (gh:#1175)**: runs; result in Sec 3.3.1 | | | | | | |
| MESH-10CM (3729 cells, gh:#1175) | 287 s | -448 pcm @ 3710 s | 8206 s | 9044 s | 25.1 % @ 9225 s | 142 kW | 0.098 kg/s |

Reading it:

- **Which numbers to quote:** RZ-BASE. The numerical uncertainty is
  ~+-60 s on the re-criticality from the time step and ~~~+260 s from 20 to
  15 cm; neither touches the ~3400 s miss. The 10 cm mesh does not run
  (unresolved; the gas Picard's convergence measure has a floor there).~~
  **CORRECTED 2026-11-03 (gh:#1175):** the mesh is **not converged**: from
  20 to 10 cm the re-criticality moves **+590 s** (7616 -> 8206 s) and the
  late power **+33 kW** (109 -> 142 kW), and the steps grow (Sec 3.3.1).
  The mesh error does not close the re-criticality miss (it widens it, to
  ~4100 s at 10 cm) but it is a third of the late-power gap. RZ-BASE stays
  at 20 cm so every case in this record compares at one resolution; the
  10 cm numbers are the better estimate of the converged answer.
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

### 3.3.1 Mesh study and the 10 cm gas Picard (gh:#1175, 2026-11-03)

**The failure, diagnosed before fixing.** On the 10 cm mesh the gas-flow
Picard (`flow::solve_gas`) failed at the first step with the loop stopped
(a reproducer is `gas_picard_converges_on_the_10cm_mesh_with_the_loop_stopped`,
12 s of coastdown then stopped). Its convergence measure was the per-cell
change of the mass flux `|dG| / max(|G|, 1e-3 kg/(m^2 s))`. The residual
history of that step:

- passes 1-30: geometric fall, factor ~0.52 per pass (the 0.5 relaxation),
  from 2.0 to 3.1e-7: **no period-2 oscillation**, no Rayleigh-Darcy limit
  cycle as in the plena of gh:#1150;
- passes 31-200: a random walk between 1.0e-7 and 7.0e-7, the cell that
  sets the maximum hopping between unrelated cells (hot-plenum-side open
  volume, bottom-reflector channels): **a floor**.

Two sources, measured on that step: in the open volumes the change was 1-3
times `T_f eps (|p_a| + |p_b|) / A_f`, the flux error of a single rounding
of the ~63 Pa hydrostatic pressure on a low-resistance face (round-off);
in the laminar bottom-reflector channels (`Re ~ 1`) the flux, ~2e-4
kg/(m^2 s), is the difference of pressures ~1e4 times the column's own
drop, so the CG's 1e-13 relative residual leaves ~5e-10 of noise, read
against the 1e-3 floor as up to 7e-7. Both are noise in `G` in cells
whose resistance does not depend on `G` (laminar limit, `f |G|` constant),
so they cannot be iterated away: raising the pass cap to 1000 (tried
before this change) could not help. The 20 cm mesh passed only because
its faces are wider (smaller transmissibility, less noise).

**The fix: measure what is iterated.** The Picard iterates
`G -> R(|G|) -> p -> G` and the pressure solve is linear given `R`, so the
convergence measure is now the relative change of the cell resistances,
`max |R(G_new) - R(G_old)| / R(G_old)`, at the **same 1e-7 tolerance**
(`flow::resistance_change`). Where `R` depends on `G` (turbulent channels,
the KTA bed) `|dln R / dln G| <= 1`, so this is no looser there; in one
coastdown step it was the stricter of the two measures throughout. Result
on the reproducer: 27 passes at the first stopped step, 13-19 per step
after; on the 20 cm mesh RZ-BASE is unchanged to every printed digit
(7616 s, 109 kW, 22.4 % at 8705 s), and so is MESH-30CM.

**The mesh study** (`rz_lofc_mesh_check`, RZ-BASE with only the largest
cell changed; Fig 2.5 lines kept, each interval split into
`ceil(length / max)` cells):

| max cell | cells | in the bed (r x z) | to 1 % | min rho | rho = 0 | first peak | mean P, last 30 min | wall |
|---|---|---|---|---|---|---|---|---|
| 30 cm | 1210 | Fig 2.5: 6-14 x 18 cm | 288 s | -370 pcm @ 3075 s | 7616 s | 22.4 % @ 8705 s | 109 kW | 48 s |
| 20 cm (RZ-BASE) | 1452 | Fig 2.5: 6-14 x 18 cm | 288 s | -370 pcm @ 3080 s | 7616 s | 22.4 % @ 8705 s | 109 kW | 53 s |
| 15 cm | 2136 | 6-14 x 9 cm | 287 s | -408 pcm @ 3350 s | 7876 s | 23.6 % @ 8930 s | 122 kW | 81 s |
| 10 cm | 3729 | 5-10 x 9 cm | 287 s | -448 pcm @ 3710 s | 8206 s | 25.1 % @ 9225 s | 142 kW | 163 s |
| 5 cm (one extra try) | 11 770 | - | **failed**: the steady solid-gas outer iteration (`solve_steady`, 0.5 relaxation) did not converge in 400 passes (last pass: 5 solid, 22 gas Picard passes); not diagnosed | | | | | 112 s |

**Convergence statement.** 30 and 20 cm are the same discretisation in the
bed (every Fig 2.5 bed interval is under 20 cm), and they agree to 1 s:
outside the bed, refining the reflectors from 30 to 20 cm changes nothing
measurable. In the bed the three distinct meshes refine different
directions (20 -> 15 cm halves the bed's axial 18 cm intervals; 15 -> 10 cm
halves its radial ones), so they are **not** a geometric sequence and no
Richardson extrapolation or observed order can be formed from them. What
the numbers do show: every metric after the minimum moves the same way on
both refinements, and the steps do not shrink (re-criticality +260 s then
+330 s; late power +13 then +20 kW; minimum -38 then -40 pcm). **The
20 cm results are not mesh-converged; the discretisation error of
RZ-BASE is at least -590 s on the re-criticality and -33 kW on the late
power, with the converged values beyond the 10 cm ones.** The short-term
metrics are converged (1 % time 288 / 287 / 287 s). A uniform-ratio
sequence (e.g. 20 / 10 / 5 cm with every interval split) needs the 5 cm
steady state to converge first.

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
   patch). Not tuned, not tested. **TESTED 2026-11-03 (gh:#1173, Sec 7):**
   the convection is now in the model (default, derived `h` 1.9-2.5
   W/(m^2 K)) and adds **~5 kW**, not 45-90 kW (that estimate held the RPV
   temperature fixed; the in-vessel path limits the sink); re-criticality
   +15 s, late power +0 to +2 kW. **The hypothesis is ruled out as the
   cause; the gap stays open.** The largest movement seen is numerical:
   109 -> 142 kW of late power from 20 to 10 cm cells (Sec 3.3.1).
2. Short-term fall and CRW peak (3.4): dummy-pebble lag hypothesis untested.
3. Riser loop and plena buoyancy not represented (3.1).
4. ~~The 10 cm mesh's gas Picard does not converge (3.3).~~ **RESOLVED
   2026-11-03 (gh:#1175):** a floor in the convergence measure, not a
   non-converging iteration; measured on the resistances now (3.3.1).
   **New, from the same study:** the 20 cm mesh is not converged (-590 s,
   -33 kW against 10 cm, steps not shrinking), and the 5 cm steady
   solid-gas outer iteration does not converge in 400 passes (3.3.1).
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
~3 min; mesh check ~6 min (four meshes, gh:#1175).

## 7. RPV-cavity natural convection (gh:#1173): predictions, written before the runs (2026-11-03)

Committed in its own commit before any coupled run with the convection.
The code (`EnclosureNaturalConvection`, `SolidBoundary::RadiativeAndConvective`,
`Htr10RzOptions::cavity_convection`, ablation `without_cavity_convection`)
was written and unit-tested first; the only numbers seen were the unit
tests' (the coefficient against a hand value, and a coarse 21 x 39 steady
case at a fixed 100 kW, whose RCCS side share rose by 1.07 kW).

**What the benchmark says.** [T1] Sec 2.1.1.4: "The RPV transfers heat to
the water cooling panels through radiation and natural convection"; the
panels are held at 50 degC. No effective coefficient is given, and the
specification and the as-run plant agree (both have the convection), so
rule D4 does not split anything here: the convection is the base case and
`CAVITY-NC-OFF` (radiation only, every record before this) is the ablation.
GAMMA+ and the CFD participants modelled the cavity air explicitly ([T1]
Sec 2.2: air up at ~0.40 m/s by the RPV, down at ~0.32 m/s by the panels,
at mid-core, steady FPIC); that is a code result, not a measurement, and
is not used.

**Model (derived, not fitted).** MacGregor & Emery (1969),
`Nu_L = 0.046 Ra_L^(1/3)` for a vertical enclosure, on the RPV-to-panel gap
`L = 2.854 - 2.18 = 0.674 m`, `H = 10.16 m` (`H/L = 15`, inside 1-40), air
at 1 atm (Assumed) at the film temperature (Sutherland / White, ideal gas,
Incropera Tab A.4 `c_p`; within 2 % of CoolProp at 300-600 K), curvature as
two wall layers in series, `x 2/(1 + 2.18/2.854) = 1.134`. Out of range:
air's `Pr = 0.7` (< 1), and `Ra_L` reaches 1.2e9 (> 1e9) at a 200 degC wall.

**Predicted coefficient** (from the code, at a 50 degC panel): **2.08 / 2.50
/ 2.74 W/(m^2 K)** for an RPV wall at 100 / 150 / 200 degC (`Ra_L` 7.2e8 to
1.2e9). At 3 h the RCCS duty of RZ-BASE (97.4 kW with the heads) puts the
area-mean RPV wall at ~130 degC (`T^4` mean of the 92 kW side duty over
139 m^2 at `eps_eff = 0.7626`), so `h ~ 2.3 W/(m^2 K)` area-mean.

**Predicted extra heat removal at 3 h.** Two estimates, the second the one
predicted:

- *RPV temperature held* (the reasoning of Sec 4 item 1, which gave
  45-90 kW): `2.3 x 139 m^2 x ~80 K ~ 26 kW`. This is an upper bound: it
  assumes the in-vessel path can supply whatever the wall rejects.
- *With the in-vessel resistance* (predicted): the radiation alone is a
  linearised `~8.4 W/(m^2 K)`; the convection adds ~28 % to the outer
  conductance. The outer path is a small part of the core-to-panel
  resistance: the gh:#1149 record's `RCCS-SHIELD` cut the radiative
  conductance to 0.56 of the base and moved the duty only -7.8 %. Scaling
  that, +28 % of outer conductance gives **+3 to +5 % of the ~100 kW, i.e.
  +3 to +6 kW at 3 h** (band +2 to +10 kW), with the RPV wall **10-20 K
  cooler**.

**Predicted shifts** (RZ-BASE and CRW-BASE, 20 cm, everything else
unchanged):

| Quantity | RZ-BASE now | Predicted | CRW-BASE now | Predicted |
|---|---|---|---|---|
| Short term (1 % time, P(100 s); CRW first peak) | 288 s, 0.162 | unchanged (< 1 s, < 0.001) | 158.4 % @ 23 s | unchanged (< 0.5 point) |
| Re-criticality | 7616 s | **earlier by 50-400 s** | 6041 s | **earlier by 50-400 s** |
| Mean power, last 30 min | 109 kW | **+2 to +10 kW** | 135 kW | **+2 to +10 kW** |
| Heat to RCCS + heads at 3 h | 97.4 kW | +2 to +10 kW | - | - |

**So the prediction is that cavity convection does not close the ~80 kW
gap**: it is the right physics and the specification's, but its leverage is
small because the heat sink is limited inside the vessel, not at the wall.
The mesh study of Sec 3.3 (this change set, gh:#1175) already shows a larger
lever on the late power: 109 kW at 20 cm, 142 kW at 10 cm.

**The gh:#1149 / gh:#1150 records move too** (the default changed, so they
are re-measured): A-MEAN950 total duty +2 to +5 % (RCCS side more, heads
less); A-DECAY 200 kW hottest cell 5-15 K cooler; FPIC outlet < 0.2 K and
thermocouple RMS < 0.5 K (the FPIC heat loss is ~1 % of 10 MW).

### 7.1 Results (2026-11-03, after the predictions above were committed in `cc9e00a09`)

`cargo test --release -p nee_soon --lib htr10_th_transient::tests_rz::rz_cavity_convection_cases -- --nocapture`
(`rz_cavity_convection_cases`; RPV wall probes at r 216 cm). RZ-BASE and
CRW-BASE are the cases of Sec 3 with the convection (the new default);
`CAVITY-NC-OFF` is the old record, reproduced to every digit.

| case | 1 % | P(100 s)/P0 | min rho | rho = 0 | first peak | mean P, last 30 min | RCCS + heads at 0 / 1 / 2 / 3 h | RPV wall z 90 at 3 h |
|---|---|---|---|---|---|---|---|---|
| RZ-BASE (20 cm) | 288 s | 0.1621 | -372 pcm @ 3085 s | **7631 s** | 22.4 % @ 8720 s | **109 kW** | 111.2 / 107.5 / 104.5 / **102.3 kW** | 132.2 degC |
| CAVITY-NC-OFF (20 cm) | 288 s | 0.1622 | -370 pcm @ 3080 s | 7616 s | 22.4 % @ 8705 s | 109 kW | 104.3 / 101.5 / 99.1 / 97.4 kW | 144.5 degC |
| RZ-BASE-10CM | 287 s | 0.1612 | -451 pcm @ 3715 s | 8211 s | 25.2 % @ 9230 s | 141 kW | 114.4 / 110.2 / 106.7 / 104.2 kW | 132.4 degC |
| CAVITY-NC-OFF-10CM | 287 s | 0.1613 | -448 pcm @ 3710 s | 8206 s | 25.1 % @ 9225 s | 142 kW | 107.0 / 103.8 / 101.1 / 99.1 kW | 144.7 degC |

| CRW case | first peak | decay level | rho = 0 | P(7200 s) | mean P, last 30 min | RCCS + heads at 3 h |
|---|---|---|---|---|---|---|
| CRW-BASE | 158.4 % @ 23 s | 272 s | **6056 s** | 727 kW | **137 kW** | 102.2 kW |
| CRW-CAVITY-NC-OFF | 158.4 % @ 23 s | 272 s | 6041 s | 694 kW | 135 kW | 97.3 kW |

**Against the predictions:**

| Quantity | Predicted | Measured | Verdict |
|---|---|---|---|
| `h` area-mean at 3 h | ~2.3 W/(m^2 K) | not printed per face; the RPV wall is 92 / 132 / 130 degC at z -200 / 90 / 400 cm at 3 h, i.e. `h` 1.9-2.4 W/(m^2 K) by the coefficient (`rpv_cavity_convection_hand_value` prints 2.08 at 100 degC, 2.50 at 150 degC) | consistent |
| Extra heat removal at 3 h | +3 to +6 kW (band +2 to +10) | **+4.9 kW** (20 cm), +5.1 kW (10 cm), +4.9 kW (CRW) | **met** |
| RPV wall | 10-20 K cooler | **12.3 K** cooler at z 90 | met |
| Short term | unchanged | 1 % time and P(100 s) unchanged to 1e-4; CRW first peak unchanged | met |
| Re-criticality | **earlier** by 50-400 s | **later by 15 s** (LOFC 20 cm and CRW), +5 s (10 cm) | **miss, wrong sign** (the size, ~0, is inside the "small" verdict) |
| Late power | +2 to +10 kW | 0 kW (LOFC 20 cm), -1 kW (10 cm), +2 kW (CRW) | miss, low |
| The gap closes? | no | no | met |

**Why the sign was wrong (measured, not argued).** The reactivity
breakdown, LOFC 20 cm, with against without the convection:

| t | moderator pcm | reflector pcm | total pcm |
|---|---|---|---|
| 1800 s | -288.8 / -288.0 | +11.1 / +11.6 | -349.9 / -348.5 |
| 3600 s | -264.2 / -263.1 | +7.9 / +8.7 | -367.1 / -365.0 |
| 7200 s | +66.6 / +67.0 | +1.7 / +3.2 | -60.3 / -58.4 |

The feedback is measured from each run's own steady state, and the
convection lowers that whole state by ~1 K (moderator 592.1 -> 591.2 degC
at 1800 s, reflector 312.3 -> 311.2 degC). After the trip the bed's own
cooling is the same to 1 pcm, while the reflectors, which give up most of
the extra 5 kW, cool ~0.2 K more against their start, and their
coefficient is positive (+7.71 pcm/K). Net ~2 pcm more negative at
7200 s, so ~15 s later. The prediction counted only the bed.

**Reading it.**
- The convection is the specification's physics and is now the default;
  it moves the duty by the predicted ~5 kW and the RPV wall by 12 K, and
  it does **not** move the transients: re-criticality +15 s, late power
  0 to +2 kW. **The ~80 kW heat-sink gap of Sec 4 item 1 is not the cavity
  air.** The leading hypothesis's 45-90 kW held the RPV temperature fixed;
  the in-vessel resistance (about 5 K/kW against ~0.9 K/kW outside the
  RPV) limits the sink.
- **What does move the late power is the mesh** (Sec 3.3.1: 109 -> 142 kW
  from 20 to 10 cm), and the bed conductivity (BED-ZBS 164 kW, Sec 3.3).
  Neither is a free parameter; the mesh is a numerical error to remove,
  not a lever.
- The gh:#1149 and gh:#1150 records were re-measured under the new default
  (their READMEs, CORRECTED 2026-11-03): A-MEAN950 RCCS 102.53 -> 105.07 kW
  (+2.5 %, total +1.2 %, predicted +2 to +5 %: a miss, low); A-DECAY hottest
  -2.8 K (predicted 5-15 K: a miss, smaller); FPIC outlet -0.52 K
  (predicted < 0.2 K: a miss), RMS 49.4 -> 49.8 K, inside the participants'
  range 22 -> 19 of 22.

## 8. Short-term power and the CRW first peak (gh:#1174): audit and predictions, written before the runs (2026-11-03)

Committed in its own commit before any coupled run with the changes below.
The code was written and unit-tested first (`transient_tests.rs`: coarse
21 x 39 cases only); the only numbers seen were the unit tests'.

### 8.1 Audit of the kinetics inputs

| Input | Model | Source | Audit |
|---|---|---|---|
| `beta_eff` | 0.00726 | [T1] Sec 2.3.1.1 (re-read 2026-11-03: "Fraction of delayed neutrons: beta = 0.726 %"), [J09] Tab 3, [C09] Tab 1, [H06] Sec 2.1 | agrees with every source |
| `Lambda` | 1.68e-3 s | [T1] Sec 2.3.1.1 ("Prompt generation time"); the TAC-NC entry of [T1] Sec 2.3.2.5 uses the same as "prompt lifetime" | agrees; [H06]'s 1.68e-4 s is `KIN-LAMBDA-H06` |
| six delayed groups | Keepin U-235 thermal, rescaled to `beta_eff` | none: [T1] and the participants say "six groups" and print none | a named assumption (kinetics.rs); U-235 thermal is the physical choice for 17 % UO2 in a graphite spectrum |
| `alpha_f / alpha_m / alpha_r` | -2.13 / -16.2 / +7.71 pcm/K | [T1] Sec 2.3.1.1, [J09] Tab 3 | agrees; the [G02] and [H06] sets are sensitivities (Sec 3.3), never picked for fit |
| zone weighting | power-weighted over the heated cells | [T1] Sec 2.1.1.6: "subdivided into all core zones according to their power fraction" | as specified |
| what **fuel** reads | fuel-zone node (volume mean of the 5 cm fuel zone) | [T1] Sec 2.1.1.6: "1 degC variation of fuel temperature within the whole core" | consistent (TRISO kernels sit at the local matrix temperature to ~1 K) |
| what **moderator** reads | ~~the bed cell: pebble graphite **outside** the fuel zones (shells and dummies at one temperature)~~ | [T1] Sec 2.1.1.6: "similar to" the fuel's; no further definition | **gap found**: the fuel zones' matrix is a third of the pebble graphite of the mixed zone (`w_f = x (2.5/3)^3 = 0.330`) and was left out; and the dummies (`w_d = 0.430`) were held at the shells' temperature |

### 8.2 Changes (code written; defaults chosen on physics before any run)

- **`ModeratorTemperature::AllPebbleGraphite` (default):** the moderator
  coefficient reads the volume mean of all pebble graphite of a heated
  cell, `0.330 T_f + 0.240 T_shell + 0.430 T_dummy`, power-weighted across
  cells. The moderator is the graphite, so all of it; which graphite [T1]'s
  coefficient means is not stated (Assumed; it is VSOP's fuel/moderator
  split). `MOD-OUTSIDE-FZ` keeps the part B reading.
- **`DummyPebbleModel::Lagged` (default):** a dummy node per heated cell
  (transient.rs module docs item 4): capacity `1 - x = 0.430` of the bed's,
  conductance per dummy `1/(1/(20 pi k R) + 1/(2 pi D k_bed + h A))`.
  Hand time constant: capacity ~317 J/K, `k_bed ~ 7-9 W/(m K)` at
  550-600 degC -> `2 pi D k_bed ~ 3 W/K`, gas film `h A ~ 3.9 W/K` at the
  3 MW flow: **~45 s** while the loop runs, **~100 s** stagnant. The unit
  test (coarse mesh, hottest cell, 2x power for 10 s): fuel zone +9.9 K,
  shells +5.7 K, dummy +0.47 K. `DUMMY-IN-CELL` is the ablation.

### 8.3 Predictions (each before measuring; base = RZ-BASE / CRW-BASE after gh:#1173)

**An energy argument settles most of the dummy-lag hypothesis before any
run.** With the all-graphite reading the moderator temperature of a cell
is (to the heat capacities' small temperature dependence) the energy
stored in its pebble graphite over its capacity: how that energy is
shared between fuel zones, shells and dummies does not move it. So the
dummy lag can only act through the fuel coefficient (`T_f` leads more
when the dummies lag: a slightly *faster* fall) and through the gas. **It
would slow the fall only under the part B reading**, which leaves out the
fuel zones: there the dummies are 0.43/0.67 of what the coefficient sees.

| Case | One change | LOFC P(100 s) (now 0.162, measured 0.204) | LOFC re-criticality | CRW first peak (now ~158 % @ 23 s) |
|---|---|---|---|---|
| KIN-LAMBDA-H06 | `Lambda` 1.68e-4 s | < 0.5 % change (prompt jump: `|rho| < beta` throughout) | < 20 s | < 1 point |
| KIN-DN-TEHO | `teh-o-prke`'s U-235 groups | within +-3 % | < 100 s | < 3 points |
| KIN-DN-1G | one group, same mean life | **lower** by 10-40 % (no 80 s tail) | < 300 s | +-10 points |
| MOD-ALLGRAPHITE only (dummies in the cell) | the moderator reading | **higher by 3-12 %** (0.167-0.182): at power the fuel zones run ~7 K (power-weighted, Sec 3.1) above the shells; as the power falls that excess goes into the cell, which the part B reading counts as a moderator rise and the all-graphite reading does not (`0.33 x 0.84 x 6.6 K ~ 1.8 K`, ~+30 pcm at 100 s) | within +-150 s | **lower by 3-10 %** and <= 1 s earlier (the same excess grows with the power: ~-20 pcm at the peak) |
| DUMMY-LAG with MOD-ALLGRAPHITE (the new base) vs MOD-ALLGRAPHITE alone | the dummy node | within +-3 % (energy argument) | within +-150 s | within +-3 points |
| DUMMY-LAG with MOD-OUTSIDE-FZ (the hypothesis of Sec 3.4 as stated) | the dummy node, part B reading | **higher by 10-30 %** | within +-300 s | **higher by 10-30 points, 2-8 s later** |

**Net prediction for the new base** (both defaults): LOFC P(100 s)
0.167-0.182 (still below 0.204), 1 % time 5-20 s later (288 -> 293-308 s);
CRW first peak 140-155 % at 21-24 s (further from 216/241 %, not closer);
re-criticality and late power within +-150 s and +-5 kW. **So the
physically consistent changes are predicted to move the LOFC the right
way and the CRW peak the wrong way, and to close neither.** If the
measured short-term power is an ex-core detector signal (Sec 3.4 (d)),
none of this is testable against it.
