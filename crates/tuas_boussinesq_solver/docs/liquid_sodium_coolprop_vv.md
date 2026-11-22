# Liquid sodium and liquid-metal Nusselt correlations in TUAS: V&V record

EBR-II groundwork (epic gh:#1312), general capability only: no EBR-II report
data is used. Items: liquid sodium as a TUAS liquid (gh:#1320) and named
liquid-metal Nusselt correlations (gh:#1321). The pump coast-down (gh:#1322)
lives in `tampines` and is recorded in the doc comments of
`crates/tampines/src/components/pump_coastdown.rs`.

AI-drafted (Claude Opus 5.5 in Claude Code), 2026-11-10. Untrusted until a
human reviews it. Research, education and V&V only.

## 1. Liquid sodium (`liquid_database::liquid_sodium_coolprop`)

### Methodology

- **Model.** `liquid_sodium()` returns a TUAS `CustomLiquid` whose four
  property callbacks call `outram_park_fork_coolprop::incompressibles::Incompressible::LiqNa`.
  No new correlation is written.
- **Range.** 400 K to 1000 K. CoolProp declares `LiqNa` valid to 2500 K, but
  its heat-capacity source data stop at 1000 K, and TUAS integrates the heat
  capacity for enthalpy, so above 1000 K every enthalpy and every temperature
  recovered from it would rest on a cubic extrapolated 1500 K beyond its
  data.
- **Reference.** CoolProp's own source table for the fit: class
  `LiquidSodium` in `dev/incompressible_liquids/CPIncomp/PureFluids.py`,
  CoolProp commit `0e67fe74b30a` (the commit the port was generated from).
  Upstream gives no literature source for the table (`reference = "LiqNa"`).
- **Checks** (all in the test module of `liquid_sodium_coolprop.rs`):
  1. TUAS's getters return `LiqNa`'s values bit for bit at 400, 550, 700,
     850 and 1000 K (wiring and units).
  2. Every getter errors at 399 K and at 1001 K.
  3. TUAS's enthalpy differences (20-point Gauss-Legendre integral of
     `c_p` from 400 K) equal the port's analytic integral to 1e-12 relative.
  4. The fit against its source table at the seven points 400-1000 K.
  5. Sanity: Pr(700 K) between 4e-3 and 6e-3.

### Results (2026-11-10)

Fit minus table, relative, at the table's temperatures in range:

| T (K) | density | heat capacity | conductivity | viscosity |
|---|---|---|---|---|
| 400 | +0.874 % | +0.014 % | -0.000 % | -2.003 % |
| 500 | +0.167 % | -0.043 % | -0.003 % | +1.287 % |
| 600 | -0.248 % | +0.038 % | +0.006 % | +1.756 % |
| 700 | -0.625 % | -0.007 % | -0.001 % | +1.582 % |
| 800 | -0.639 % | +0.011 % | +0.002 % | +0.830 % |
| 900 | -0.675 % | -0.023 % | -0.002 % | -0.015 % |
| 1000 | -0.521 % | +0.010 % | +0.003 % | -0.402 % |

These are CoolProp's own fit residuals: the port reproduces the fit exactly
(check 1). The density cubic is loosest at the low end (+0.87 % at 400 K);
the viscosity form misses by up to 2 %. Checks 1-3 and 5 pass. The bounds
asserted in check 4 (density 1 %, heat capacity 0.1 %, conductivity 0.01 %,
viscosity 2.1 %) were set **after** this measurement, as a regression pin on
the residuals; no acceptance band for sodium properties has been set.
Pr(700 K) = 5.0e-3.

Above the range, for the record (not used): the extrapolated heat capacity
is 1352 J/(kg K) at 1400 K and 2658 J/(kg K) at 2500 K; the density fit
misses its table by -4.1 % at 2400 K and +11.0 % at 2500 K.

### Interpretation

Sodium is selectable in TUAS (`Material::Liquid(liquid_sodium())`) and in
`tampines` (`tampines::single_phase::liquid_sodium`). The properties are
CoolProp's, with the residuals above against CoolProp's table; the table's
own provenance is **Not re-checked** (no source is recorded upstream). A
comparison against an independently published sodium property set (for
example Fink and Leibowitz 1995, ANL/RE-95/2) has not been made.

## 2. Liquid-metal Nusselt correlations (`nusselt_number_correlations::liquid_metal`)

### Methodology

- **Correlations.** Lyon-Martinelli and Seban-Shimazaki (circular tube) and
  Kazimi-Carelli, Schad-modified and Mikityuk (triangular rod bundle), in
  `Pe = Re Pr`. Formulas, sources and validity ranges are in the module doc.
- **Upstream.** Four of the five follow MOOSE thermal-hydraulics
  `ADWallHeatTransferCoefficient{Lyon,Kazimi,Schad,Mikityuk}Material.C`
  (LGPL-2.1, commit `e316801b544a`), read for the formula and its guards
  (`Pe` clamped to at least 1; ranges warned, not enforced; Schad's branch
  at `Pe < 150`). Seban-Shimazaki has no upstream code in reach.
- **Uncalibrated.** Published coefficients, unchanged.
- **Reference values.** Each formula evaluated in a separate Python 3
  one-liner at `Pe = 200`, `P/D = 1.25` (and Schad at `Pe = 100`).
  Pass criterion: relative difference at most 1e-5.

### Results (2026-11-10)

| Correlation | Python reference | Code | Pass |
|---|---|---|---|
| Lyon-Martinelli | 8.73286 | 8.73286 | yes |
| Seban-Shimazaki | 6.73286 | 6.73286 | yes |
| Kazimi-Carelli | 5.88677 | 5.88677 | yes |
| Schad-modified, Pe = 200 | 8.28622 | 8.28622 | yes |
| Schad-modified, Pe = 100 | 7.60105 | 7.60105 | yes |
| Mikityuk | 8.91005 | 8.91005 | yes |

Also verified: the `Pe < 1` clamp; Schad's branches meet at `Pe = 150` to
1e-4; `NusseltCorrelation::LiquidMetal` and its four `estimate_*` routes give
the same number as the free function; the validity check flags `P/D = 1.6`
(Mikityuk) and `Pe = 2000` (Schad).

### Interpretation and findings

- This verifies the implementation of the formulas, not their accuracy for
  any bundle. No comparison against sodium heat-transfer data has been made.
- **Upstream documentation defect:** MOOSE's page for the Mikityuk material
  prints the bundle factor as `1 - exp(P/D - 1)`; its code uses
  `1 - exp(-3.8 (P/D - 1))`, which is the form used here.
- **TUAS low-Peclet axial conduction.** TUAS adds fluid axial conduction only
  when `Pe < 100`, and its own comment calls that branch "quite buggy". For
  sodium (Pr about 5e-3) that is `Re` below about 2e4, which a pump
  coast-down reaches. That branch is unverified for liquid metals.
