# MSRE library module: methodology and results (gh:#1330)

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). AI draft
> (Claude Opus 5.5 in Claude Code), unreviewed. An offline demonstration
> model, never for operation, licensing or safety decisions.
> **Status: NOT VALIDATED.**

Module: `outram_park_digital_twin_engine::msre` (`src/msre/`). Through the
`outram-park` metacrate it is
`outram_park::backend::outram_park_digital_twin_engine::msre`.

Why it is in this crate's library: maintainer direction 2026-11-10 (gh:#1317,
"I would like to have MSRE and EBR-II in library so I can build web
simulations from the outram-park metacrate"), a sanctioned exception to the
crate's "no new physics in the library" rule, on the `htr10` pattern.

## What the module is

| Path | Content | Origin |
|---|---|---|
| `msre::design` | Cited plant constants: the ORNL-TM-0728 design point, TM-1070 kinetics and lump coefficients, the TM-1626 measured circulation loss | Here; TM-1070 values re-exported from `nee_soon` |
| `msre::model::MsreModel` | Stepping simulator | Here, composing `teh-o-prke` and TM-1070's equations |
| `msre::frequency_response` | MSRE T-1 linear model, TM-1647 measured periods | `nee_soon` re-export |
| `msre::first_criticality` | MSRE N-1 full-core Monte Carlo model | `outram-mc-libs` re-export |
| `msre::circulating_kinetics` | MSRE T-C1 circulating point kinetics | `teh-o-prke` re-export |
| `msre::salts` | MSRE TH-6 fuel and coolant salts | `tuas_boussinesq_solver` re-export |

## Methodology

### The model

`MsreModel` integrates in time the equations `nee_soon::msre_frequency_response`
solves in the frequency domain (ORNL-TM-1070, Ball and Kerlin, 1965), with
the same coefficients, read from `nee_soon`, not copied:

- **Kinetics:** `teh_o_prke`'s `CirculatingPointKinetics` (T-C1), unchanged:
  six groups from TM-1070 App. B p. 69, `Lambda = 2.4e-4 s`, core transit
  8.46 s, loop transit 16.73 s (Table 1). Nonlinear (`rho * n`), backward
  Euler.
- **Core:** two fuel lumps and a graphite lump, (A.1)-(A.3), App. B
  pp. 67-68. Reactivity follows fuel lump 1 and the graphite.
- **Delayed gamma:** 5.64 % of the power through a 188 s lag (Table 1).
- **Heat exchanger and radiator:** (A.32)-(A.37), App. B p. 69; the radiator
  air side scaled with air flow as TM-1070 p. 60 scales it.
- **Piping:** pure delays, App. B p. 70 (5.77, 8.67, 4.71, 8.24 s).
- **Coupling:** operator splitting, reactivity from the previous step's
  temperatures. Fixed internal step 0.01 s; every lag is a whole number of
  steps. `step(dt)` runs as many as fit and carries the remainder.

### What is new physics, run uncalibrated

1. **Absolute temperatures.** TM-1070's model is a deviation model. The same
   linear equations carry absolute temperatures once one boundary value is
   fixed: the radiator air inlet, **100 °F** (TM-0728 Table 8.2). The steady
   state is then a 14-unknown linear solve with every pipe's outlet equal to
   its inlet.
2. **Feedback reference:** the initial steady state, critical with no rod
   reactivity; the kinetics run at `rho = circulation loss + rods +
   feedback`.

No coefficient was adjusted against any comparison below.

### Not modelled

Pump trips or any flow change (the transits are fixed; variable-transit
circulating kinetics would belong in `teh-o-prke`, follow-up gh:#1332), xenon, drain tank and freeze valve, spatial effects, decay heat
beyond the delayed-gamma lag. A lumped core gives averages, not peaks.

## Predictions (posted on gh:#1330 before any run)

1. Steady critical reactivity = the analytic circulation loss with TM-1070
   transits, 0.2467 % dk/k, +16 % against the measured 0.212 ± 0.004 %.
2. Time-domain amplitude and phase at the resonance within 2 % and 2° of
   `nee_soon`, at 7.3 MW and 1 MW.
3. Free damped oscillation period after a small step at 7.3 MW within ±15 %
   of TM-1647's 1.71 min.
4. Uncalibrated 10 MW steady state: core outlet within ±75 °F of 1225 °F,
   radiator air outlet within ±50 °F of 300 °F.
5. TUAS fuel salt `rho c_p Q` at 1200 gpm within 2 % of TM-1070's
   `MC_F / tau_F`.

## Results (2026-11-10, branch `feat/engine-msre-lib` from develop fef1ebdea)

Run with
`cargo test --release -p outram-park-digital-twin-engine --lib msre -- --nocapture`.
Each test's doc comment carries the same numbers.

### 1. Circulation loss (MSRE N-4 quantity): predicted, met

| | Loss (% dk/k) |
|---|---|
| This model (TM-1070 transits) | **0.2467041** |
| `nee_soon` (same equation) | 0.2467041 (equal within 1e-12) |
| Measured, ORNL-TM-1626 | 0.212 ± 0.004 |
| Difference | **+0.0347, +16.4 %, +8.7 sigma** |

The lumped, flat-importance core counts the plenum-born delayed neutrons as
worthless and reads high; the same model with TM-1626's transits gives
0.2301 % (N-4, gh:#1130). Not tuned. Also checked: the delayed-neutron
sources sum to `beta - loss` to 1e-12.

### 2. Code-to-code against the frequency-domain model: predicted, met

Rod reactivity driven at 1e-5 dk/k, sinusoidal, at `nee_soon`'s resonance;
start-up transient discarded (3000 s, 5000 s); projection over four whole
periods.

| P [MW] | omega [rad/s] | time domain abs(H), phase | `nee_soon` abs(H), phase | ratio, phase difference |
|---|---|---|---|---|
| 7.306 | 0.061109 | 1219.79, +10.703° | 1219.45, +10.687° | 1.00027, +0.016° |
| 1.004 | 0.015466 | 5011.19, +19.361° | 5012.74, +19.374° | 0.99969, −0.013° |

`H` is `(dP/P0)/d(rho)`. Agreement to 3e-4 in amplitude and 0.02° in phase,
against the 2 % / 2° posted. This checks the time integration, the
absolute-temperature form, the splitting and the nonlinearity together.

### 3. Natural period at 7.306 MW (MSRE T-1 quantity)

Measured as TM-1647 measured it: `2 pi / omega` at the peak of the response,
here the vertex of a parabola in `ln omega` through the three highest of five
time-domain amplitudes (periods 0.85 to 1.15 of `nee_soon`'s peak).

| | Natural period [min] |
|---|---|
| This model, time domain | **1.713** |
| `nee_soon`, frequency domain | 1.714 |
| TM-1647 Fig. 29, measured (digitised, about ±2 %) | 1.71 |
| Difference to measured | **+0.2 %** |

With TM-1070's 1965 predicted coefficients (fuel −4.84, graphite −3.7 ×
10⁻⁵ /°F). With N-3's Monte Carlo coefficients `nee_soon` records 2.03 min
(+18.8 %) at this power (gh:#1132). One power only here; the full seven-power
comparison is `nee_soon`'s record
(`crates/nee_soon/verification_and_validation/msre_t1_frequency_response/README.md`).

**Prediction 3 missed.** The free-oscillation peak spacing after a +1e-5
step is **2.346 min, +37 %** against 1.71 min, outside the ±15 % predicted.
The prediction, not the model, was wrong: the response is heavily damped, and
the spacing of a step response's first two maxima is not the quantity TM-1647
measured. It is printed by the test as a record, not asserted.

### 4. Uncalibrated steady state at 10 MW against TM-0728: predicted, met

| | model [°F] | TM-0728 [°F] | model − design [°F] |
|---|---|---|---|
| core inlet (fuel) | 1201.33 | 1175 | +26.33 |
| core outlet (fuel) | 1257.73 | 1225 | **+32.73** |
| HX coolant inlet | 1068.46 | 1025 | +43.46 |
| HX coolant outlet | 1153.46 | 1100 | +53.46 |
| radiator air outlet | 309.75 | 300 | **+9.75** |
| graphite, nuclear average (TM-0728 §2.4.1 estimate) | 1259.53 | ~1255 | +4.5 |

The whole plant sits 26-53 °F hot. Part is structural: TM-1070's fuel
heat-capacity rate gives a 56.4 °F core rise against TM-0728's 50 °F, and the
HX coolant rises 85 °F against 75 °F. The rest is the level TM-1070's radiator
coefficients put the coolant at over a 100 °F air inlet. TM-0728's values are
the design heat balance, not measurements (the MSRE ran at about 8 MW).
Pinned to 0.01 °F as a regression guard, not a criterion.

### 5. Heat-capacity rate cross-check (TH-6 against T-1's lumps): predicted, met

| | `rho c_p Q` [W/K] |
|---|---|
| TUAS fuel salt (TM-0728 Table 2.1) × 1200 gpm | 3.1978e5 |
| TM-1070 `MC_F / tau_F` | 3.1915e5 |
| Difference | **+0.20 %** |

For the record: TUAS's coolant salt (ORNL-TM-2316) at 850 gpm gives
2.5240e5 W/K, a 71.3 °F rise at 10 MW against TM-0728's 75 °F.

### Harness checks (not V&V)

- Steady 300 s at 8 MW: worst `abs(P/P0 - 1)` = 3.3e-16, worst temperature
  drift 1.4e-12 °F.
- Core energy balance: core rise equals `P tau_F / MC_F` to 1e-10 at 0.5, 5,
  10 MW.
- Radiator air flow cut to half at 10 MW, 3000 s: no heat against the
  gradient at any step; settles at 6.8625 MW, net reactivity 3.6e-13.
- Rod +20 pcm at 5 MW (N-3 coefficients): 5.355 MW at 1 s, peak 6.035 MW,
  5.015 MW at 601 s.
- Bit-identical reruns; the step remainder is carried.
- **Cost: 0.68 µs per 0.01 s step** (about 15 000 × real time), one core of
  the development container, 2026-11-10. Machine-dependent; not asserted.

## Interpretation

The time-domain model is a faithful integration of TM-1070's equations
(result 2), so it inherits the T-1 record's agreement with the measured
natural periods (result 3, and `nee_soon`'s seven-power table) and the
lumped kinetics' known +16 % circulation-loss excess (result 1). The new
part, the absolute temperature level, is 26-53 °F above TM-0728's design
heat balance, uncalibrated (result 4). It is a reduced-order teaching and
demonstration model, not a validated simulator.
