# GeN-Foam `twoPhase` port: V&V record (gh:#1218, epic gh:#1216)

**Status: predictions committed before any run (2026-11-04).** The results
sections below are filled in by later commits; this file's git history shows
the predictions were written first.

## What is compared, and against what

The AGENTS.md differential-testing rule asks for the port to be run against
upstream on shared cases, upstream first where it is runnable.

**Upstream cannot be run here.** The disk has about 0.2 GB free; OpenFOAM
v2506 plus GeN-Foam needs several GB to build (the build recorded in
`docs/genfoam-tutorial-verification.md`, 2026-09-15, lived on a `/workspace`
volume that no longer exists). So the comparison is against what upstream
**ships**, case by case:

| case | what upstream ships as a reference | selects models the port has? |
|---|---|---|
| `1D_boiling` | **nothing numeric.** `Alltest` only checks the log ends in `End` | yes, except the alpha solver (`MULES`; see below) |
| `1D_PSBT_SC/Phase_Ex1_12223` | `Alltest`: `alpha.vapour` = 0.131886 at t = 0.6 s, line 98 of the field file, ±5 % | no: `multiRegimeBoilingTRACE`, `twoParameters` regime map, `byRegime` area/diameter |
| `1D_PSBT_SC/PhaseII_Ex1_01_5215` | `regressionResults`: exit `alpha.vapour` 0.123835, `T.fixedPower` 620.178; plus the 2026-09-15 upstream run's t = 5 s fields in `tests/data/genfoam_psbt_fs_drag.csv` | no, same gaps |
| `1D_CHF` | nothing numeric (`printResults.py` plots only) | no: `multiRegimeBoilingTRACECHF`, `multiRegimeBoilingVapourTRACE`, `CachardLiquid`/`CachardVapour` |
| `2D_voidMotionNoPhaseChange` | nothing numeric | alpha solver `MULES` only |
| `2D_KNS37-L22` | experimental data (KNS-37 test L22), not upstream output | not attempted this session |

**A finding in the shipped references:** `1D_PSBT_SC/regressionResults` line 1
records exit `alpha.vapour` 0.133727 while `Phase_Ex1_12223/Alltest` expects
0.131886, and its line 4 prints "0.133727 ... It should be 0.319074". The file
mixes runs, so it is not used as a reference; the `Alltest` values are.

## Predictions for `1D_boiling` (written before the port was run)

The case: 40 cells in x (`low` 10, `mid` 20, `top` 10; 2 m), structure
volume fraction 0.5, sodium liquid (`icoPolynomial`/`hPolynomial`/`polynomial`)
and vapour (`perfectGas`/`hConst`/`const`), `p_rgh` fixed at 1.4 bar (inlet)
and 1.0 bar (outlet), `g = (-9.81 0 0)`, `fixedPower` 4.5e8 W/m^3 on structure
volume fraction 0.5 in `mid`, power off by `timer` at 7 s, 12 s run.

**P1. Initial single-phase flow: closed form, tight.** With uniform density the
buoyancy flux term vanishes, so the steady face-momentum balance is
`alpha dp_rgh/dx = Kd U` per zone with `Kd = 0.5/Dh (1 - alpha_s) rho |U| f`,
`f = 2 Re^-0.125` (ReynoldsPower), `Re = |U| Dh_s / nu`. At 652.15 K the
case's polynomials give rho = 862.276 kg/m^3, mu = 2.88033e-4 Pa s,
Cp = 1283.07 J/(kg K), kappa = 70.65 W/(m K). Solving
`0.4e5 = (U/alpha) sum_z Kd_z L_z` gives **U = 0.71198 m/s** (Re = 10 657 in
`mid`). The port should reach it to **1e-3 relative** before boiling starts
(the residual is the transient not quite settled plus the 1e-6 solver
tolerances; both terms are small at 0.7 s of physical settling time). A miss
larger than 1e-3 is a port defect in the momentum/pressure path.

**P2. Boiling must start before power-off.** Steady single-phase outlet
temperature rise would be `Q/(mdot Cp)` = 2.25e5 W / (0.30696 kg/s x
1283.07) = **571 K**, i.e. 1223 K, above the Browning-Potter saturation
temperature (1152.6 K at 1 bar, 1173.1 K at 1.2 bar). The structure heats at
up to 117 K/s adiabatic. Predicted: first vapour (`alpha.vapour > 1e-3`) in the
upper part of `mid` **between 1.5 s and 5 s**. Outside that window is a
finding to explain.

**P3. Flow excursion.** Being pressure-driven, the inlet liquid flow should
fall once vapour forms (two-phase friction via the Kaiser88 multiplier and the
vapour's acceleration). Predicted: inlet velocity falls below 50 % of P1's value
during boiling. Direction is the prediction; the magnitude is not, because
nothing upstream ships to compare it with.

**P4. Power-off at 7 s, then condensation.** After 7 s the structure stops
heating; vapour should condense and `max alpha.vapour` should decrease
monotonically to below 1e-3 before 12 s, and the inlet flow should recover
towards P1.

**P5. Conservation.** Cumulative continuity errors (upstream's
`calcCumulContErrs`) should stay below 1e-3 kg/m^3 over the run. Energy: before
boiling, structure power in = enthalpy flux out + storage, checked to 1e-3
relative in a near-steady window.

**What would not count as agreement.** None of P2-P4 is a comparison with
upstream. They test that the port behaves like the physics it ports. Parity
with upstream on `1D_boiling` is **not** measured by this record and must not
be claimed from it.

## Deviations from the case, decided before the run

- **Alpha solver.** `1D_boiling` selects `MULES`, which is not ported. The
  port runs GeN-Foam's other alpha solver, `implicitUpwind`
  (`solveAlphaImplicitUpwind.H`), with `solvePhase vapour`. This changes the
  alpha discretisation (first-order upwind, implicit, no sub-cycling) and is a
  known source of difference from an upstream run.
- **Linear solvers.** `p_rgh` uses ILU(0)-preconditioned BiCGStab where upstream
  uses GAMG; both are run to the case's absolute tolerance, so the difference is
  below the stated solver tolerance.
