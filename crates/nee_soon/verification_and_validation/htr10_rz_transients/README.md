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

> **CORRECTED 2026-11-03 (gh:#1173, gh:#1174): two defaults changed after
> this section was written** (RPV-cavity air convection; the moderator
> coefficient on all pebble graphite with a dummy-pebble node). The tables
> of Sec 3 are the part B record as run; the same cases under the current
> defaults are in **Sec 9** (headline: re-criticality ~~7616 s~~ 7426 s,
> P(100 s) ~~0.162~~ 0.170, CRW first peak ~~158.4 %~~ 154.4 % at 22 s,
> late power 109 kW unchanged). The plots in this folder show the current
> base.

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
**UPDATED 2026-11-03 (gh:#1176, Sec 10):** done. The 5 cm steady failure
was a non-settling outer iteration in the discharge tube's gas (the
lagged-density map oscillates), fixed by a density-consistent gas solve
in the steady state; the ratio-2 sequence 20 cm x 1 / 2 / 4 gives the
re-criticality 7426 / 8246 / 8571 s, **observed order 1.34, Richardson
~8780 s**, and the heat to the RCCS and heads at 3 h 102.3 / 104.4 /
104.7 kW (Richardson 104.8 kW). The axial part of the drift is the
upwind gas cell (first order); the radial part is not attributed.

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
Unknown; it needs a two-temperature bed. **TESTED 2026-11-03 (gh:#1174,
Sec 8):** the two-temperature bed exists now (default); with the moderator
read on all the pebble graphite the dummy lag moves P(100 s) by -3 % and
the CRW peak by -2.6 points (an energy argument predicted ~0), so it is
not the cause. It does act as stated (+12 %, CRW 175 % at 32 s) only if
the coefficient leaves the fuel zones' graphite out (`MOD-OUTSIDE-FZ`).
The audit found that reading in the part B model and replaced it.

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
   cause; the gap stays open.** ~~The largest movement seen is numerical:
   109 -> 142 kW of late power from 20 to 10 cm cells (Sec 3.3.1).~~
   **CORRECTED 2026-11-03 (gh:#1176, Sec 10.3):** the mesh is not the gap
   either: the heat to the RCCS and heads at 3 h is mesh-converged at
   ~104.8 kW (102.3 kW at 20 cm); the late *mean power* moves with the
   mesh only because the post-return oscillation shifts. ~~**Open:** the
   in-vessel path (~80 kW).~~ **CORRECTED 2026-11-04 (gh:#1173), from the
   thermocouples already in Sec 3.2, no new run:** the side wall at the two
   instrumented heights is **not** the bottleneck. The flux through the side
   carbon brick, from the TC pair across it (SR3/SR2, SR9/SR8; r 167 -> 189
   cm) and the Tab 2.12 brick conductivity at the pair's mean temperature,
   cylindrical conduction:

   | z, time | measured dT / flux | model dT / flux | model / plant |
   |---|---|---|---|
   | 80 cm, t0 | 20.6 K / 508 W/m^2 | 32.2 K / 798 W/m^2 | 1.57 |
   | 80 cm, 3 h | 29.7 K / 736 W/m^2 | 49.9 K / 1249 W/m^2 | 1.70 |
   | 170 cm, t0 | 46.7 K / 1173 W/m^2 | 31.8 K / 788 W/m^2 | 0.67 |
   | 170 cm, 3 h | 50.1 K / 1267 W/m^2 | 59.5 K / 1501 W/m^2 | 1.18 |

   At 3 h the model already drives **more** heat through the side wall than
   the plant at both heights (1.18x, 1.70x), the opposite of an 80 kW
   in-vessel deficit there. The steady FPIC profile agrees (the model's drop
   from r 93 to 193 cm at z 170 is 271 K against 272 K measured;
   `outram-foam-appbuilder-lib` `htr10_rz_flow_fpic` README). The primary
   loop is not the missing path either: the secondary is isolated at 12 s
   (Sec 7.1 of the input sheet). **Limits of the instrument:** two heights
   only; the brick conductivity is the spec's, the same on both sides of the
   comparison, so a plant brick that conducts better than Tab 2.12 would not
   show; TC positions are read from a schematic (Uncertain, item 5).
   **What remains, each untested:** (a) the premise. "~180 kW" is the
   plant's *power* at 3 h ([C09] Sec 5); it equals the heat loss only once
   the plant is steady, and the internals are still heating at 3 h (TR1&TR4
   +172.5 K in 3 h), so part of it may be storage. The model at 3 h is near
   steady (power 109 kW, heat out 104 kW). (b) A heat path above or below
   the instrumented heights (top head, bottom head, the cone and discharge
   tube region), where no thermocouple constrains the model. (c) The
   measured power itself at ~2 % of full power: the ex-core signal is
   already an open item for gh:#1174 (item 2).
2. ~~Short-term fall and CRW peak (3.4): dummy-pebble lag hypothesis untested.~~
   **UPDATED 2026-11-03 (gh:#1174, Sec 8):** kinetics inputs audited
   (`Lambda`, groups ruled out); moderator reading corrected to all pebble
   graphite and the dummy lag modelled (both default). LOFC P(100 s)
   0.162 -> 0.170 (measured 0.204); CRW first peak 158 -> 154 % at 22 s
   (measured 216-241 % at 30-35 s). **Both misses remain.** Open: which
   graphite the -16.2 pcm/K coefficient belongs to (MOD-OUTSIDE-FZ gives
   175 % at 32 s; measurable with `htr10_rmc::zones`), the ex-core signal,
   the decay-heat history.
3. Riser loop and plena buoyancy not represented (3.1).
4. ~~The 10 cm mesh's gas Picard does not converge (3.3).~~ **RESOLVED
   2026-11-03 (gh:#1175):** a floor in the convergence measure, not a
   non-converging iteration; measured on the resistances now (3.3.1).
   **New, from the same study:** the 20 cm mesh is not converged (-590 s,
   -33 kW against 10 cm, steps not shrinking), and the 5 cm steady
   solid-gas outer iteration does not converge in 400 passes (3.3.1).
   **UPDATED 2026-11-03 (gh:#1176, Sec 10):** the steady iteration is
   fixed (density-consistent gas); the ratio-2 study gives order 1.3 and
   a converged re-criticality ~8800 s (20 cm: -1350 s of mesh error). Still
   open: the first-order upwind gas exchange temperature (axial part),
   the radial part (not attributed), and the lagged gas of a transient
   step, which alternates in the discharge tube on the finest mesh.
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
~3 min; mesh check ~6 min (four meshes, gh:#1175); since gh:#1176
`rz_lofc_geometric_mesh_study` ~1 h (REF-4X 50 min) and
`rz_lofc_mesh_diagnostics` ~6 min. Since gh:#1173 / gh:#1174 also
`rz_cavity_convection_cases` (~10 min, Sec 7) and
`rz_short_term_feedback_cases` (~12 min, Sec 8).

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

### 8.4 Results (2026-11-03, after the predictions above were committed in `c9c78b6fb`)

`cargo test --release -p nee_soon --lib htr10_th_transient::tests_rz::rz_short_term_feedback_cases -- --nocapture`
(20 cm, cavity convection ON; BASE = both new defaults; PARTB = both
ablated, which reproduces the Sec 7 base to every digit).

**LOFC** (relative fission power; measured from the digitised [C09] Fig 6):

| case | 1 % | P 20 s | P 50 s | P 100 s | P 200 s | min rho | rho = 0 | first peak | mean P, last 30 min |
|---|---|---|---|---|---|---|---|---|---|
| **BASE** | **296 s** | 0.870 | 0.446 | **0.170** | 0.036 | -348 pcm @ 3000 s | **7426 s** | 20.3 % @ 8530 s | 109 kW |
| DUMMY-IN-CELL | 296 s | 0.877 | 0.461 | 0.176 | 0.036 | -353 @ 3050 | 7466 s | 21.2 % @ 8560 | 109 kW |
| MOD-OUTSIDE-FZ | 287 s | 0.877 | 0.482 | 0.181 | 0.034 | -393 @ 2995 | 7746 s | 26.0 % @ 8835 | 88 kW |
| PARTB (Sec 7 base) | 288 s | 0.855 | 0.433 | 0.162 | 0.033 | -372 @ 3085 | 7631 s | 22.4 % @ 8720 | 109 kW |
| KIN-LAMBDA-H06 | 296 s | 0.865 | 0.442 | 0.169 | 0.036 | -346 @ 3005 | 7416 s | 20.0 % @ 8505 | 108 kW |
| KIN-DN-TEHO | ~~297 s~~ **279 s** | ~~0.870~~ **0.861** | ~~0.446~~ **0.413** | ~~0.171~~ **0.148** | ~~0.036~~ **0.029** | ~~-348 @ 3000~~ **-327 @ 3045** | ~~7431 s~~ **7311 s** | ~~20.3 % @ 8530~~ **19.9 % @ 8335** | ~~109~~ **106 kW** |
| KIN-DN-1G | 217 s | 0.902 | 0.523 | 0.167 | 0.015 | -346 @ 3005 | 7416 s | 22.9 % @ 8660 | 115 kW |
| *measured* | ~330 s | 0.905 | 0.496 | **0.204** | 0.047 | - | 2520 / 4075 s | 24.6 % @ 4395 s | ~180 kW |

**CRW:**

| case | first peak | P 30 s | P 35 s | to decay level | rho = 0 | mean P, last 30 min |
|---|---|---|---|---|---|---|
| **BASE** | **154.4 % @ 22 s** | 1.485 | 1.408 | 286 s | **5771 s** | 125 kW |
| DUMMY-IN-CELL | 157.0 % @ 23 s | 1.537 | 1.470 | 279 s | 5901 s | 126 kW |
| MOD-OUTSIDE-FZ | **175.0 % @ 32 s** | 1.746 | 1.731 | 254 s | 6511 s | 136 kW |
| PARTB (Sec 7 base) | 158.4 % @ 23 s | 1.547 | 1.477 | 272 s | 6056 s | 137 kW |
| KIN-LAMBDA-H06 | 155.6 % @ 21 s | 1.483 | 1.399 | 286 s | 5751 s | 126 kW |
| KIN-DN-TEHO | ~~154.4 % @ 22 s~~ **161.1 % @ 21 s** | ~~1.485~~ **1.529** | ~~1.408~~ **1.430** | ~~286 s~~ **272 s** | ~~5771 s~~ **5586 s** | ~~125~~ **142 kW** |
| KIN-DN-1G | 139.3 % @ 31 s | 1.393 | 1.379 | 232 s | 5751 s | 137 kW |
| *measured* | 216 % @ 35 s [H06] / 241 % @ 30 s [J09] | - | - | 810 s | 2400 / ~3400 s | ~200 kW |

**Against the predictions of Sec 8.3:**

| Case | Predicted | Measured | Verdict |
|---|---|---|---|
| KIN-LAMBDA-H06 | P(100 s) < 0.5 %; CRW < 1 point | -0.6 %; +1.2 points | marginal miss (still negligible) |
| KIN-DN-TEHO | +-3 %; < 3 points | ~~+0.3 %; 0.0 points~~ **-13.3 %; +6.7 points** (re-measured after gh:#1319) | ~~met~~ **miss since gh:#1319** (the prediction assumed the old set, which was Keepin's in all but rounding) |
| KIN-DN-1G | P(100 s) lower by 10-40 % | -2.2 % (but P(200 s) -59 %, 1 % at 217 s); CRW -15 points, 9 s later | miss: the one-group set crosses the six-group curve near 100 s; not a candidate either way |
| MOD-ALLGRAPHITE alone (DUMMY-IN-CELL vs PARTB) | P(100 s) +3 to +12 %; CRW peak -3 to -10 %; rho = 0 +-150 s | **+8.6 %**; **-1.4 points** (-0.9 %); -165 s (LOFC), -155 s (CRW) | met; CRW sign met, size a miss (smaller); re-criticality just outside |
| DUMMY-LAG, all-graphite (BASE vs DUMMY-IN-CELL) | +-3 %; +-3 points; +-150 s | -3.2 %; -2.6 points; -40 s / -130 s | met (P(100 s) at the edge) |
| DUMMY-LAG, part B reading (MOD-OUTSIDE-FZ vs PARTB) | +10 to +30 %; +10 to +30 points, 2-8 s later | +11.8 %; **+16.6 points, 9 s later** | met (timing just outside) |
| Net new base | P(100 s) 0.167-0.182; 1 % 293-308 s; CRW 140-155 % @ 21-24 s; rho = 0 and late power +-150 s / +-5 kW | **0.170; 296 s; 154.4 % @ 22 s**; rho = 0 **-205 s** (LOFC) / **-285 s** (CRW); late power 0 / **-12 kW** (CRW) | short term met; re-criticality miss (larger, toward the measurement); CRW late power miss (away) |

**The energy argument, checked.** At 100 s into the LOFC the moderator
reading is 591.00 degC with the dummy node and 591.43 degC without it
(0.4 K, 7 pcm): the dummy lag does not move the all-graphite moderator
temperature, as argued. What it moves is the fuel term (-36.9 against
-25.9 pcm): with the dummies lagging, the fuel zones run hotter (597.3
against 592.2 degC). Under the part B reading the same lag moves the
moderator reading by 2.5 K (588.5 degC).

**Reading it.**
- **Ruled out as causes:** `Lambda` (< 1 %), ~~the delayed-group set (the
  Keepin and `teh-o-prke` U-235 sets agree to 0.3 %; one group is not
  physical)~~, and the dummy-pebble lag under the all-graphite reading
  (< 3 %, and the wrong way).
- **CORRECTED 2026-11-10 (gh:#1319): the delayed-group set is not ruled
  out.** `teh-o-prke`'s U-235 set is now ENDF/B-VIII.0 (`sum beta_i /
  lambda_i` 0.08030 s at beta 0.00726, against Keepin's 0.09449 s; the old
  `teh-o-prke` set was 0.09473 s, Keepin's in all but rounding, which is
  why it moved nothing). With it the LOFC P(100 s) is **0.148** (BASE
  0.170, measured 0.204: further from the measurement) and the CRW first
  peak **161.1 % at 21 s** (BASE 154.4 %, measured 216-241 %: closer).
  The two evaluated sets bracket a 13 % spread in P(100 s), so the
  short-term LOFC comparison carries a delayed-data uncertainty of that
  size. BASE still uses Keepin (the named base assumption); this is
  recorded, not a reason to switch. Re-measured with
  `cargo test --release -p nee_soon --lib rz_short_term_feedback_cases --
  --nocapture`; predictions were posted on gh:#1319 before the run
  (P(100 s) -3 to -10 %: a miss in size, -13 %; CRW peak +1 to +6 points:
  just over, +6.7; re-criticality within +-150 s: miss for CRW, -185 s;
  late power within +-5 kW: miss for CRW, +17 kW).
- **What the corrected physics does:** reading the moderator coefficient
  on all the pebble graphite takes P(100 s) from 0.162 to **0.170**
  (measured 0.204: ~20 % of the gap closed), the 1 % time from 288 to
  296 s (measured ~330 s), and brings the LOFC re-criticality 205 s and
  the CRW's 285 s earlier. It lowers the CRW first peak slightly, 158.4 ->
  **154.4 %** (measured 216-241 %), still at 22 s against 30-35 s. **Both
  short-term misses remain.**
- **The one case that moves the CRW peak towards the measurement is
  MOD-OUTSIDE-FZ** (the dummy lag with the part B reading: 175 % at
  32 s, and P(100 s) 0.181). It gets there by leaving the fuel zones'
  matrix, a third of the pebble graphite, out of the moderator. **It is
  not adopted because it is closer** (the defaults were fixed in Sec 8.2
  before the run). Whether [T1]'s -16.2 pcm/K belongs to all the pebble
  graphite or mostly to the dummies and shells is a neutronics question
  this record cannot settle; it can be settled with the workspace's own
  Monte Carlo model: `nee_soon::htr10_rmc::zones` already separates the
  fuel-pebble and moderator-pebble temperatures, so a `ParametricStudy`
  can measure the coefficient of each (follow-up).
- **Still untested:** the measured short-term power is an ex-core signal
  (3.4 (d)); the bed heat capacity; the decay heat's saturated-history
  assumption (kinetics.rs); the Tab 2.60 reactivity shape.

## 9. The record re-measured under the current defaults (2026-11-03, after gh:#1173-#1175)

`cargo test --release -p nee_soon --lib --tests -- --nocapture` (the full
suite), with the defaults of gh:#1173 (cavity convection) and gh:#1174
(all-graphite moderator reading, dummy-pebble node). The tables of Sec 3
are the gh:#1151 part B record and stay as written; the plots in this
folder (`rz_lofc_*.png`, `rz_crw_*.png`) were regenerated by this run and
show the current base.

**RZ-BASE** (20 cm): 1 % at **296 s** (gate 330 +- 120 s: passes; part B
288 s); minimum -348 pcm at 3000 s; re-criticality **7426 s** (part B
7616 s; measured 2520 / 4075 s); power back above 1 % at 8338 s; first
peak 20.3 % at 8530 s (measured 24.6 % at 4395 s); mean power, last
30 min, **109 kW** (measured ~180 kW). Short term, model / measured:
10 s 0.987 / 0.989; 20 s 0.870 / 0.905; 50 s 0.446 / 0.496; 100 s
**0.170** / 0.204; 200 s 0.036 / 0.047; 300 s 0.0095 / 0.0139; 400 s
0.0029 / 0.0056. RCCS + heads 111.2 kW at 0, 102.3 kW at 3 h.
Thermocouples: RMS over all 17 traces and bins **32.2 K** (part B 32.4 K),
mean +7.4 K; TR1&TR4 274 -> 493 degC (+219 K; measured +173 K); side
reflector within 45 K everywhere.

**LOFC sensitivity set** (one change each from RZ-BASE):

| case | to 1 % | min rho | rho = 0 | power back > 1 % | first peak | mean P, last 30 min |
|---|---|---|---|---|---|---|
| RZ-BASE | 296 s | -348 pcm @ 3000 s | 7426 s | 8338 s | 20.3 % @ 8530 s | 109 kW |
| NC-OFF | 301 s | -320 @ 211 s | 5041 s | 6724 s | 5.7 % @ 7020 s | 70 kW |
| SRC-OFF | 296 s | -348 @ 3000 s | 7426 s | 9448 s | 122.6 % @ 9510 s | 201 kW |
| SRC-INTRINSIC | 296 s | -348 @ 3000 s | 7426 s | 8692 s | 38.3 % @ 8825 s | 90 kW |
| FUEL-QS | 307 s | -332 @ 3030 s | 7346 s | 8261 s | 18.5 % @ 8455 s | 108 kW |
| REFL-SIDE | 300 s | -322 @ 232 s | 6641 s | 7571 s | 19.8 % @ 7765 s | 134 kW |
| BED-ZBS | 303 s | -315 @ 227 s | 5136 s | 6068 s | 20.5 % @ 6260 s | 166 kW |
| BYP-G02 | 297 s | -342 @ 2945 s | 7326 s | 8233 s | 20.5 % @ 8425 s | 110 kW |
| BYP-NOLEAK | 292 s | -398 @ 3310 s | 8241 s | 9187 s | 18.2 % @ 9385 s | 119 kW |
| TR-SPEC | 306 s | -313 @ 2865 s | 7411 s | 8373 s | 20.2 % @ 8580 s | 100 kW |
| KIN-ALPHA-G02-EQ | 387 s | -249 @ 3490 s | 9336 s | 10716 s | 3.1 % @ 10800 s | 36 kW |
| XE-OFF | 298 s | -325 @ 236 s | 6356 s | 7281 s | 20.1 % @ 7475 s | 157 kW |
| DT-HALF (0.5 / 2.5 s) | 297 s | -348 @ 2980 s | 7373 s | 8280 s | 19.8 % @ 8470 s | 109 kW |
| CAVITY-NC-OFF (gh:#1173) | 297 s | -346 @ 2990 s | 7416 s | 8325 s | 20.2 % @ 8515 s | 109 kW |
| DUMMY-IN-CELL / MOD-OUTSIDE-FZ / PARTB (gh:#1174) | Sec 8.4 | | | | | |

**Mesh** (`rz_lofc_mesh_check`): 30 / 20 / 15 / 10 cm: re-criticality
7426 / 7426 / 7696 / **8036 s**; minimum -348 / -348 / -385 / -424 pcm;
first peak 20.3 / 20.3 / 21.4 / 22.7 %; mean power, last 30 min,
108 / 109 / 111 / **164 kW**; 1 % at 296 / 296 / 295 / 295 s. The same
picture as Sec 3.3.1: converged in the short term, not converged after
the minimum (+610 s on the re-criticality from 20 to 10 cm; the steps
+270 s then +340 s do not shrink). The late-power metric jumps at 10 cm
(111 -> 164 kW) because the last 30 min then fall on a different part of
the post-return oscillation: read it with the re-criticality, not alone.

**CRW set:**

| case | first peak | to decay level | min rho | re-criticality | re-criticality peak | P at 7200 s | mean P, last 30 min | TR1&TR4 rise at 2 h |
|---|---|---|---|---|---|---|---|---|
| CRW-BASE | **154.4 % @ 22 s** (5034 kW; 170.6 % of 3000 kW) | 286 s | -276 pcm @ 300 s | **5771 s** | 19.4 % @ 6930 s | 138 kW | 125 kW | +224 K |
| CRW-TRIP9 | 142.8 % @ 20 s | 288 s | -269 @ 304 s | 5736 s | 19.2 % @ 6900 s | 117 kW | 125 kW | +224 K |
| CRW-SRC-OFF | 154.4 % @ 22 s | 286 s | -276 @ 300 s | 5771 s | 82.4 % @ 7680 s | 35 kW | 131 kW | +217 K |
| CRW-NC-OFF | 154.3 % @ 22 s | 290 s | -264 @ 264 s | 2816 s | 8.9 % @ 4415 s | 176 kW | 77 kW | +9 K |

Measured: first peak 216 % at 35 s [H06] / 241 % at 30 s [J09]; decay
level 810 s; re-criticality 2400 s [H06] / ~3400 s [T1]; long-term
~200 kW; top reflector +215 K [H06].

**What changed from part B, in one line each:** the short term moved
towards the plant in the LOFC (P(100 s) 0.162 -> 0.170) and slightly away
in the CRW (158 -> 154 %); the re-criticality came 190 s (LOFC) and 270 s
(CRW) earlier, still ~3400 s late; the late power did not move (LOFC
109 kW) or fell (CRW 135 -> 125 kW); the thermocouples are unchanged
(RMS 32.2 K). The miss list of Sec 4 stands, with items 1, 2 and 4
updated.

## 10. Mesh convergence (gh:#1176): a geometric sequence; predictions, written before the runs (2026-11-03)

Committed in its own commit before any coupled run on the new meshes. The
code (`Htr10RzOptions::refinement`, `TransientCoreOptions::with_refinement`;
`rz_lofc_geometric_mesh_study`, `rz_lofc_mesh_diagnostics`) was written
first; the only numbers seen are the steady-state diagnosis of 10.1.

**Why a new sequence.** The `max_cell` sequence of Sec 3.3.1 splits each
Fig 2.5 interval into `ceil(length / max)` cells, so 20 -> 15 cm halves the
bed's axial intervals only and 15 -> 10 cm its radial ones: no ratio, no
order. `refinement = [f, f]` splits **every** cell of the 20 cm mesh
`f x f`: `REF-1X` (1452 cells, the RZ-BASE mesh itself), `REF-2X` (5808),
`REF-4X` (23 232), a ratio-2 sequence with every region boundary on a face
(the Fig 2.5 lines stay mesh lines). Time step, kinetics and every other
input unchanged.

### 10.1 The finest steady state: diagnosis (before any transient)

`steady_outer_iteration_on_the_finest_mesh` (appbuilder, `long-tests`)
prints every pass of `Htr10RzTransientCore::solve_steady` on `REF-4X` at
the as-run 3.315 MW / 1.457 kg/s state (new `SteadyOuterPass` history:
largest solid and gas change, the cell that set it, the sub-solves'
passes). Measured, 2026-11-03 (log kept at
`logs/steady_outer_iteration_ref4x.log`):

- passes 0-86: a monotone geometric fall, factor ~0.92-0.94 per pass, from
  ~60 K to 0.35 K; the largest change moves from the bed to the discharge
  tube (region 30) and stays there; solid and gas change together;
- pass 87: the **gas** change jumps (0.37 -> 0.67 K, then 1.7, 4.5, 7.0 K)
  at the axis cells of the discharge tube, r 0.8-4 cm, z 281-293 cm (the
  tube level beside the hot plenum, z 295-325), while the solid change is
  still 0.3 K; the gas-flow Picard rises from 15 to 21-23 passes;
- passes 90-400: a bounded wander, solid 2-4 K and gas 5-9 K, the worst
  cell hopping among the same axis cells: **neither a floor nor a
  divergence: a non-settling (limit-cycle-like) outer iteration**, started
  in the gas of the discharge tube.

The 5 cm `max_cell` failure of Sec 3.3.1 ended at the same signature (gas
Picard 22 passes on its last pass). Reading: the tube is a pebble column
(KTA, buoyancy on) 25 cm in radius, heated from the side by the hot
plenum and the bottom reflector; with 16 radial cells instead of 4 it
resolves a radial circulation in the tube, and the outer iteration, in
which the flow sees the gas densities of the previous pass, does not
settle it. Whether a steady state exists at all there is tested next
(10.2), before any fix.

### 10.2 Predictions (each before its run)

**The steady state (before the march).** `finest_mesh_march_at_constant_power`
marches the physical transient at the same constant state from the last
iterate (1 s, 10 s, 100 s steps). Predicted: the march **settles** (largest
change per 100 s step below 1e-3 K within 120 steps), i.e. a steady state
exists and the block iteration, not the physics, fails to reach it. If it
does not settle, the tube flow is unsteady at this resolution and no
quasi-steady steady state exists to start a transient from.

**The geometric sequence** (`rz_lofc_geometric_mesh_study`):

| Quantity | REF-1X (= RZ-BASE) | REF-2X | REF-4X | observed order | Richardson |
|---|---|---|---|---|---|
| re-criticality | 7426 s exactly (identity) | 8000-8400 s | 8250-8900 s | 0.5-1.5 | 8400-9500 s |
| min rho | -348 pcm | -390 to -460 pcm | -410 to -500 pcm | 0.5-1.5 | - |
| RCCS + heads at 3 h | 102.3 kW | +1 to +4 kW | +1.5 to +6 kW | - | - |
| mean P, last 30 min | 109 kW | 110-170 kW | 120-190 kW | not formed if not monotone | - |
| P(100 s) | 0.170 | within 0.002 | within 0.002 | - | - |

The late mean power is a poor convergence metric (Sec 9: it can fall on a
different part of the post-return oscillation); the heat to the RCCS and
heads at 3 h and the re-criticality are the ones to quote.

**Where the mesh dependence acts** (`rz_lofc_mesh_diagnostics`, each
against REF-1X / REF-2X):

| Case | Predicted | Reasoning |
|---|---|---|
| NC-OFF 1X -> 2X | re-criticality moves by **less than 30 %** of the base's 1X -> 2X change | the solid-only gh:#1149 model converges fast (A-MEAN950 duty +1.8 % from 819 cells to 10 cm, 0.4 % after); the transient's mesh drift appeared only with the gas circulation |
| REF-R2 (radial only) | 30-70 % of the 2X change | Sec 3.3.1: both the axial-only (20 -> 15 cm, +270 s) and the radial-dominated (15 -> 10 cm, +340 s) steps moved it |
| REF-Z2 (axial only) | 30-70 % of the 2X change | as above |

**Candidate mechanisms, named before the runs** (each first order in the
cell size, which would explain a slow, monotone drift):

1. **The dropped buoyancy half-cell.** On a face between a porous or
   channel cell and an open volume the gravity term is dropped (gh:#1150),
   but the face's `dz` spans both half cells, so the porous half cell next
   to every plenum also loses its head. At the bed top (18 cm cells at
   1X) that is ~9 cm of a ~217 cm column, ~4 % of the bed's buoyancy head,
   halving with each refinement. Predicted size alone: +50 to +150 s of
   re-criticality per halving (NC-OFF moves it by ~-2400 s for the whole
   circulation): **not enough to be the whole drift**.
2. **First-order upwind gas energy** (`flow::solve_gas`): numerical
   diffusion `G c_p dx / 2` in the circulation; size not estimated.
3. **Resolution of the circulation itself**: the radial structure of the
   upflow/downflow in the bed and the side-reflector boundary layer, which
   a 6-14 cm radial cell smears.

### 10.3 Results (2026-11-03, after the predictions above were committed in `ffdc07b33`)

**The steady state: march, gas alone, fix.**

- *March* (`finest_mesh_march_at_constant_power`,
  `logs/march_constant_power_ref4x.log`): **did not settle**, against the
  prediction (miss). Over 60 steps of 1 s and 60 of 10 s the axis gas of
  the discharge tube (r 0.8-2.4 cm, z 227-289 cm) changed by 50-120 K per
  step and the solid there by 0.1-1.1 K per step, without decaying. The
  gas has no storage in this model and its flow sees the previous step's
  densities, so the march repeats the same lagged map as the steady
  iteration; it says nothing about physical stability.
- *Gas alone at fixed solid* (`finest_mesh_gas_alone_at_fixed_solid`,
  `logs/gas_alone_at_fixed_solid_ref4x.log`): repeating `solve_gas` from
  its own state oscillates by 50-100 K plain and by 8.8 K with the steady
  solve's 0.5 relaxation (the axis cell's axial flux flipping between
  -2.4e-2 and +1.3e-2 kg/(m^2 s)), and **converges with a 0.2 relaxation**
  (factor ~0.75 per repeat, to 2e-5 K in 40). **Root cause:** a
  consistent gas state exists; the fixed-point map in which the flow sees
  the gas densities of the previous pass has a slope below -1 at the axis
  cells of the tube once they are resolved (16 radial cells in 25 cm), so
  it oscillates instead of converging. Not a floor, not a divergence.
- *Fix* (`flow::solve_gas_consistent`): the gas solved consistently in its
  own density, Aitken-relaxed (Irons-Tuck), to 1e-4 K, inside the steady
  outer iteration (inexact early, tightening to 1e-4 K). **The outer
  tolerance (1e-4 K) is unchanged.** REF-4X steady: **240 passes, last
  change 9.0e-5 K** (`logs/steady_outer_iteration_ref4x_consistent_gas.log`).
  It reaches the same fixed point: REF-1X reproduces RZ-BASE to every
  printed digit.
- *Tried and not kept:* the same consistent solve at every transient
  step. On the 2x mesh it needed 15-87 evaluations per step at 1e-4 K and
  12-51 at 1e-2 K, and at the last forced-flow step of the coastdown
  (0.12 kg/s) it did not converge in 200 evaluations at either tolerance
  (residual wandering 0.05-0.4 K). **A step keeps the lagged gas**
  (THERMIX's explicit coupling), so every transient record keeps its
  scheme. **Known limitation of REF-4X:** its tube gas alternates step to
  step as in the march (the tube is unheated and below the cone; its
  effect on the metrics is not separated here).

**The geometric sequence** (`rz_lofc_geometric_mesh_study`, 4 cores
shared with the Monte Carlo of Sec 11; wall 57 / 391 / 2990 s):

| Quantity | REF-1X (1452) | REF-2X (5808) | REF-4X (23 232) | observed order | Richardson | Predicted | Verdict |
|---|---|---|---|---|---|---|---|
| re-criticality | 7426 s | 8246 s | 8571 s | **1.34** | **8784 s** | 7426 / 8000-8400 / 8250-8900; order 0.5-1.5; 8400-9500 | met |
| min rho | -348 pcm @ 3000 s | -465 @ 3635 s | -530 @ 3865 s | 0.87 | -607 pcm | -390 to -460 / -410 to -500 | miss (deeper) |
| RCCS + heads at 3 h | 102.28 kW | 104.39 kW | 104.74 kW | 2.6 | **104.8 kW** | +1 to +4 / +1.5 to +6 kW | met |
| mean P, last 30 min | 109 kW | 145 kW | 134 kW | not monotone | - | 110-170 / 120-190 | met, as warned not a convergence metric |
| P(100 s) | 0.1704 | 0.1699 | 0.1690 | - | - | within 0.002 | met |
| 1 % time | 296 s | 294 s | 292 s | - | - | - | converged to ~4 s |
| initial moderator reading | 575.54 C | 566.10 C | 560.49 C | 0.75 | ~552 C | - | - |
| first peak | 20.3 % @ 8530 s | 23.4 % @ 9260 s | 25.0 % @ 9550 s | - | - | - | - |
| max bed upflow after 1000 s | 0.092 | 0.097 | 0.101 kg/s | - | - | - | - |

**Where the mesh dependence acts** (`rz_lofc_mesh_diagnostics`, lagged gas
in the steady state, which converged on these meshes; REF-1X / REF-2X of
the table above; `logs/mesh_diagnostics_lagged_gas.log`):

| Case | re-criticality | initial moderator | Predicted | Verdict |
|---|---|---|---|---|
| NC-OFF 1X -> 2X | 5041 -> 5936 s (**+895 s**, 109 % of the base's +820 s) | 575.42 -> 565.92 C | < 30 % of the base's change | **miss**: the circulation does not carry it |
| REF-R2 (radial only) | 7951 s (+525 s, 64 %) | 575.80 C (+0.3 K) | 30-70 % | met |
| REF-Z2 (axial only) | 7731 s (+305 s, 37 %) | 565.80 C (-9.7 K) | 30-70 % | met |

The two directions add (525 + 305 = 830 s against 820 s).

**What drives it.**

1. **Axial: the upwind, well-mixed gas cell.** Each cell's gas is at its
   outlet temperature, so in forced flow a heated bed cell exchanges with
   gas half a cell's rise hotter than the gas at its centre, and its solid
   sits that much too hot. Measured (`upwind_half_cell_gas_rise_in_the_bed`,
   prediction committed before the run in `289398826`: the 1x - 2x
   difference of the half rise within +-40 % of the initial moderator's
   9.4 K): power-weighted half rise **21.77 K at 1x, 10.69 K at 2x**
   (exactly first order), difference 11.08 K against 9.44 K: **met**
   (17 %). So RZ-BASE starts ~19 K too hot in the moderator reading at
   full flow; once the flow stops the bias vanishes, and the feedback,
   measured from the biased start, sees the bed as cooler than it is: an
   early re-criticality. At -16.2 pcm/K the 1x bias is ~300 pcm of
   reference.
2. **Radial: not attributed.** Halving the radial cells moves the
   re-criticality +525 s without moving the initial state (+0.3 K); with
   NC-OFF the whole 2x change persists, so it is in the solid, fuel and
   feedback part, not the circulation. The moderator reading cools more
   slowly (594.4 against 592.0 C at 1800 s). The solid-only gh:#1149
   model converges at order ~1.2 in its duty; which radial feature
   (bed-to-side-reflector conduction, the power weighting of a peaked
   profile) carries this is not isolated. Open.
3. **The dropped buoyancy half cell** (candidate 1 of 10.2): not the
   driver (NC-OFF keeps the whole dependence).

**What it means for the record.**

- **Quote:** re-criticality **~8800 s** (Richardson, order 1.3; REF-4X
  8571 s) for the converged model, against 7426 s at 20 cm and measured
  2520 / 4075 s. **The mesh error was hiding ~1350 s of the miss**: the
  converged model is ~4700-6300 s late, not ~3400-4900 s.
- **The heat sink is converged** (104.8 kW at 3 h, order 2.6; +2.5 kW
  over 20 cm). **The ~80 kW gap of gh:#1173 is not a mesh artefact.** The
  rise of the late mean power with refinement (109 -> 145 -> 134 kW) is
  where the post-return oscillation falls, not more heat removed.
- The short term is converged (1 % time 296 / 294 / 292 s; P(100 s)
  0.1704 / 0.1699 / 0.1690).
- RZ-BASE stays at 20 cm (cost: 57 s against 50 min at 4x) so the
  sensitivity sets compare at one resolution; their numbers carry a mesh
  error of about -1350 s on the re-criticality.
- **A first-order term remains in the model**: the upwind gas cell's
  exchange temperature (item 1). Replacing it with a cell-centre gas
  temperature (an exponential or linear profile within the cell) would
  remove the axial part; it changes the gh:#1150 FPIC record too, so it
  is left as a follow-up, not done here.

## 11. Which graphite carries the moderator coefficient (gh:#1174): Monte Carlo worths; predictions, written before the runs (2026-11-03)

**Question.** [T1] gives one moderator coefficient, -16.2 pcm/K, for "the
moderator" (Sec 8.1). The R-Z core reads it as the volume mean of all
pebble graphite of a heated cell, `0.330 T_f + 0.240 T_shell + 0.430
T_dummy` (Sec 8.2, assumed). That is right only if a kelvin in a gram of
fuel-pebble graphite is worth the same as in a gram of dummy-pebble
graphite. The whole-core Monte Carlo model measures it.

**Method.** `examples/htr10_graphite_zone_worth.rs` over
`htr10_rmc::parametric::graphite_worth_cases` (new; tested by
`a_slot_off_its_zone_temperature_moves_only_that_slot`): the N = 12 Şeker
bed, rods out, every zone (gh:#1138) at **600 K**; then one part at
**1200 K** at a time:

| Case | Hot | Cold |
|---|---|---|
| BASE | - | everything |
| FUEL-GRAPHITE | the fuel pebbles' graphite (`mat::GRAPHITE`: fuel-zone matrix and shell) | kernels, coatings, everything else |
| DUMMIES | the moderator (dummy) pebbles, bed, cone and tube | everything else |
| TOP / SIDE / BOTTOM | that reflector zone | everything else |
| KERNEL | kernels and the four TRISO coatings | everything else |

Each part's nuclides are processed at its temperature (RECONR, BROADR,
HEATR, PURR) and its graphite law generated by LEAPR there
(`ParametricStudy::plan_with_slot_temperatures`, new). Worth
`1/k_BASE - 1/k_case`; statistics **3000 x [20 inactive + 80 active]**,
one seed for all (correlated sampling; the uncorrelated sigma is quoted, an
upper bound). The 600-1200 K secant is centred near the bed's ~860 K
operating moderator temperature; the coefficient's temperature dependence
is part of what is measured, not removed.

**Cost, measured on this machine before choosing the statistics:** the
N = 12 quick run (2000 x 100) took 1202 s of transport and 250 s of data
(`htr10_seker_2026_10_07/logs/run_e8_N12.log`) for sigma 272 pcm. So
3000 x 100 is ~30 min of transport per case and sigma ~230 pcm per `k`,
~320 pcm per worth: ~7 cases in ~4 h. More histories are not affordable
here alongside the transients; the top and bottom reflector worths will
not be resolved (stated in advance).

**Predictions (per kelvin, 600 -> 1200 K secant):**

| Case | Predicted | Reasoning |
|---|---|---|
| FUEL-GRAPHITE + DUMMIES | -8 to -25 pcm/K | [T1] -16.2 pcm/K at operating; the secant and the model differ |
| DUMMIES share of the pebble-graphite worth | 0.35-0.50 | their mass share in the mixed zone is 0.43; the cone and tube dummies sit where the flux is low |
| per-gram importance, fuel-pebble graphite / dummy graphite | **0.85-1.15** | the thermal diffusion length in the bed (~50 cm) is far above a pebble, so the spectrum, which carries the coefficient, is flat across pebbles |
| KERNEL | -1.5 to -3.5 pcm/K | [T1] fuel coefficient -2.13 pcm/K |
| TOP + SIDE + BOTTOM | +3 to +12 pcm/K | [T1] +7.71 pcm/K |
| SIDE share of the reflector worth | 0.5-0.85 | it faces the whole bed height |

**How the weighting follows from it (fixed now, before the numbers).**
With `I` the worth per kelvin per gram of graphite and `rho_I =
I_fuel-graphite / I_dummy` (the per-gram masses from the model's 0.57 fuel
fraction above the bed floor; the cone and tube dummies are counted with
the bed's, a stated bias towards a low `I_dummy`), the moderator reading
of a heated cell becomes

`T_mod = [x rho_I (z T_f + (1 - z) T_shell) + (1 - x) T_dummy] / [x rho_I + 1 - x]`,

`z = (2.5/3)^3`, `x` the as-run fuel fraction; the matrix and the shell
take the same per-gram importance (MC cannot split them: one slot). With
`rho_I = 1` this is today's `AllPebbleGraphite`. The predicted effect on
the transients is then written down, in its own commit, before the LOFC
and CRW runs: with `rho_I` in 0.85-1.15 the prediction is **no visible
change** (P(100 s) within +-0.003, CRW first peak within +-2 points,
re-criticality within +-100 s).

### 11.1 Results

**Run 2026-11-03 23:04 -> 2026-11-04 00:18 UTC** on develop `2bc7eab1c`,
`cargo run --release -p nee_soon --example htr10_graphite_zone_worth --
--base 600 --delta 600 --particles 3000 --inactive 20 --active 80 --threads
16`, the statistics fixed above, seed 20260917, 0 histories lost in every
case. Intel Xeon W-2195 @ 2.30 GHz, 36 logical cores, 16 threads used, CPU
only, Linux; the machine was shared with a `cargo test` run. Per case ~170-210 s
of data and 410-530 s of transport.

| Case | k | Worth (pcm) | pcm/K |
|---|---|---|---|
| BASE | 0.95389 +- 0.00229 | - | - |
| FUEL-GRAPHITE | 0.90249 +- 0.00247 | -5970 +- 394 | -9.95 +- 0.66 |
| DUMMIES | 0.91825 +- 0.00234 | -4068 +- 375 | -6.78 +- 0.63 |
| TOP | 0.95415 +- 0.00251 | +29 +- 374 | +0.05 +- 0.62 |
| SIDE | 0.96370 +- 0.00292 | +1067 +- 403 | +1.78 +- 0.67 |
| BOTTOM | 0.95580 +- 0.00222 | +210 +- 350 | +0.35 +- 0.58 |
| KERNEL | 0.94050 +- 0.00258 | -1492 +- 385 | -2.49 +- 0.64 |

The sigma is the uncorrelated one (an upper bound, as stated above).

**Against the predictions:**

| Quantity | Predicted | Measured | |
|---|---|---|---|
| FUEL-GRAPHITE + DUMMIES | -8 to -25 pcm/K | **-16.73 +- 0.91 pcm/K** | inside; [T1] -16.2 |
| DUMMIES share of the pebble-graphite worth | 0.35-0.50 | **0.405 +- 0.04** | inside |
| `rho_I`, per-gram importance fuel-pebble / dummy graphite | 0.85-1.15 | **1.11 +- 0.13** | inside, and 0.8 sigma from 1 |
| KERNEL | -1.5 to -3.5 pcm/K | **-2.49 +- 0.64 pcm/K** | inside; [T1] -2.13 |
| TOP + SIDE + BOTTOM | +3 to +12 pcm/K | **+2.18 +- 1.08 pcm/K** | **miss, low**; [T1] +7.71 is 5.1 sigma above it |
| SIDE share of the reflector worth | 0.5-0.85 | 0.82, not resolved (TOP and BOTTOM are each consistent with zero, as stated in advance) | inside |

`rho_I` uses the per-gram split fixed above: the fuel pebbles' graphite
0.57 and the dummies' 0.43 of the pebble graphite above the bed floor, the
cone and tube dummies counted with the bed's (the stated bias towards a low
`I_dummy`, i.e. a high `rho_I`): `I_fuel = 9.95 / 0.57 = 17.5`, `I_dummy =
6.78 / 0.43 = 15.8`, ratio 1.11, sigma from the two worths' relative
sigmas in quadrature.

**Reading it.** The pebble graphite carries the moderator coefficient
(-16.7 pcm/K against [T1]'s -16.2), and a kelvin in a gram of fuel-pebble
graphite is worth the same as in a gram of dummy graphite within the
statistics, as predicted from the ~50 cm thermal diffusion length. The
volume-mean reading (`rho_I = 1`, today's `AllPebbleGraphite`) is therefore
supported; the measured `rho_I = 1.11` is the default by the rule fixed
above. The reflector coefficient is the one miss: +2.2 +- 1.1 pcm/K against
[T1]'s +7.71 and the predicted +3 to +12. The secant (600 -> 1200 K, every
other zone at 600 K) is not [T1]'s operating-point derivative, and the
uncorrelated sigma overstates the noise; the miss is recorded, not
explained. It does not enter the R-Z model, which takes [T1]'s reflector
coefficient.
