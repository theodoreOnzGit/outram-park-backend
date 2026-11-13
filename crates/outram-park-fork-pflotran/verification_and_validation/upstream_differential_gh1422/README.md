# PFLOTRAN 669fbce differential and lineage audit (gh:#1422, gh:#1423)

> **Research, education and V&V only.** Not for nuclear facility operation,
> licensing, safety-critical or safeguards use (see `RESPONSIBLE_USE.md`).
> AI-generated and not yet human-reviewed.

## PREDICTIONS (written and committed BEFORE the first upstream run, 2026-11-13)

Bounds below are derived from the formulas, not from any measurement. A breach
is a finding to investigate, never a reason to move the bound.

Input conventions: upstream takes van Genuchten `m` and derives `n = 1/(1-m)`;
the port takes `n` and derives `m = 1 - 1/n`. The differential feeds the port
`n = 1/(1-m)` evaluated in f64, so for `m` that is not a dyadic rational the
port's `m'` can differ from upstream's `m` by up to 1 ulp.

| id | surface | prediction (fixed before running) |
|---|---|---|
| P1 | VG saturation `Sl(pc)`, sets with exact `m` (0.5, 0.75) | same formula, same operation order, so bit-identical except where `pow` differs by 1 ulp between glibc (upstream) and PETIR libm (port). Count pinned, worst relative <= 4.5e-16 |
| P2 | VG saturation, sets with inexact `m` (0.4, 0.2, 0.3) | `Se = f^-m`: relative sensitivity to `m` is `ln f`, at most ~25 on this grid; with `dm` <= 1.1e-16 the worst relative difference <= 1e-14 |
| P3 | upstream `pc_alpha_n < 1e-15` returns exactly 1; the port does not | affects only `pc` where `(alpha*pc)^n < 1e-15`; the port returns `1 - m*x`, so absolute difference <= 1e-15 there |
| P4 | `dSl/dP` (VG, BC) | algebraically equal, different operation order: NOT bit-identical; relative <= 1e-13. Upstream is `dSl/dP = -(1-Sr) dSe/dpc`; the port reports `dSe/dpc`, so the test applies that conversion |
| P5 | VG-Mualem `kr(Sl)` | same formula; the nested `1-(1-Se^(1/m))^m` amplifies a 1-ulp `pow` difference by `A = 1/(1-(1-x)^m)`, `x = Se^(1/m)`; per-point bound `8*eps*(1+A)`. Bit-identical where the `pow` results agree |
| P6 | VG-Mualem `dkr/dSl` | different algebraic grouping: not bit-identical; per-point bound `64*eps*(1+A)` after the `1/(1-Sr)` conversion, excluding `Se >= 0.99` where upstream is smooth-off and the port is singular |
| P7 | Brooks-Corey `Sl(pc)`, Burdine `kr(Sl)` | `(alpha*pc)^-lambda` and `Se^(3+2/lambda)`: bit-identical except libm `pow` (<= 4.5e-16 relative per `pow` call, 2 x for the exponent evaluation); bound 2e-15. Port saturates for `pc <= 1/alpha`, upstream for `pc < 1/alpha`: at equality both give 1 within 1e-15 |
| P8 | water density, IF97 Region 1 (port `eos_real` = tampines IF97 vs upstream `EOSWaterDensityIF97`) | same standard (IAPWS-IF97 Region 1), different arithmetic order: relative <= 1e-11 |
| P9 | water enthalpy, IF97 Region 1, J/kg | same standard: relative <= 1e-9 once upstream's J/kmol is divided by 18.01534 (FMWH2O). A different molar mass would show as a constant ratio, not scatter |
| P10 | saturation pressure IF97 | same standard, relative <= 1e-11 |
| P11 | upstream default density IFC67 vs the port's `LiquidWaterEos` (constant compressibility) | different model; <= 1.5 % for T <= 100 C, P <= 10 MPa. This is a model difference, not a defect |
| P12 | viscosity: upstream default `EOSWaterViscosity1` (empirical) vs port `IapwsWater::viscosity` (IAPWS) | different models: <= 5 % over 5 to 300 C |
| P13 | Debye-Huckel constants: upstream `reaction%debyeA/B/Bdot` vs port `DH_A/DH_B` | port 0.5085/0.3281 are textbook 25 C values; upstream defaults 0.5114/0.3288/0.0410. Predicted relative differences A: -5.7e-3, B: -2.1e-3 (port lower); consequently `log10 gamma` at small I differs by -5.7e-3 relative |

Route taken and the lineage audit are recorded below once the runs exist.
