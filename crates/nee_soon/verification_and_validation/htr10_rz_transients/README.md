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
