# GeN-Foam `twoPhase` port: V&V record (gh:#1218, epic gh:#1216)

**Status: predictions committed before any run (2026-11-04, commit `94bf9aa6a`);
results added below on the same day.** The results
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
be claimed from it. **CORRECTED 2026-11-12** (gh:#1361 round 3): it IS measured, elsewhere, against the real
binary: `verification_and_validation/genfoam_vs_upstream_gh1361/README.md`, "Round 3".

## Deviations from the case, decided before the run

- **Alpha solver.** `1D_boiling` selects `MULES`, which is not ported. The
  port runs GeN-Foam's other alpha solver, `implicitUpwind`
  (`solveAlphaImplicitUpwind.H`), with `solvePhase vapour`. This changes the
  alpha discretisation (first-order upwind, implicit, no sub-cycling) and is a
  known source of difference from an upstream run.
- **Linear solvers.** `p_rgh` uses ILU(0)-preconditioned BiCGStab where upstream
  uses GAMG; both are run to the case's absolute tolerance, so the difference is
  below the stated solver tolerance.

## Methodology (as run)

- **Port:** `genfoam::thermal_hydraulics::solver::two_phase` (`TwoPhaseSolver`),
  reading upstream's case files unchanged through `TwoPhaseCase::read`
  (vendored copy: `tests/data/genfoam_1d_boiling/`, see its `PROVENANCE.md`).
- **Test:** `tests/genfoam_two_phase_1d_boiling.rs`, three tests, 7.8 s in
  release for all three:
  `cargo test --release -p outram-foam-appbuilder-lib --test genfoam_two_phase_1d_boiling`.
- **Plots:** `TWO_PHASE_OUT=<dir>` on that command writes `history.csv` and
  `profiles.csv`; `python plot_1d_boiling.py <dir> .` draws the PNGs here.
- **Time stepping:** upstream's `twoPhase::maxDeltaT` (maxCo 2, maxCoTwoPhase
  0.25, marginToPhaseChange 20 K, maxDeltaT 0.01 s) and `adjustableRunTime`
  write-time hitting; 4577 steps to 12 s.
- **Deviation** (printed by the run): `alpha solver MULES is not ported; ran
  GeN-Foam's implicitUpwind (solveAlphaImplicitUpwind.H) with solvePhase vapour`.

## Results, measured 2026-11-04

| | predicted (before the run) | measured | verdict |
|---|---|---|---|
| **P1** isothermal liquid velocity vs closed form 0.711977 m/s | within 1e-3 | **0.711972 m/s at the inlet, worst cell 2.25e-5 relative** (heater ablated, t = 1 s) | **pass** |
| P1 as first stated: heated case before boiling, within 1e-3 | within 1e-3 | inlet 0.700430 m/s (**-1.62 %**), outlet 0.727761 m/s at t = 0.3 s | **miss, explained** (below) |
| **P2** first `alpha.vapour/(1-alpha_s) > 1e-3` | 1.5 s to 5 s | **6.14 s** (power off at 7 s) | **miss, explained** (below) |
| **P3** flow excursion: inlet liquid velocity < 50 % of P1 | yes (direction only) | yes: falls to 0.1 m/s by 6.5 s and **reverses**, minimum **-0.94 m/s** at 9.1 s | pass on direction; magnitude unverified |
| **P4** void condenses to < 1e-3 before 12 s | yes | max void 0.97 at 7 s, **5.9e-12 at 12 s** (gone by ~10.3 s) | pass |
| **P5** cumulative continuity error < 1e-3 kg/m^3 | < 1e-3 | liquid **-2.675 kg/m^3**, vapour 1.85e-3 kg/m^3 | **miss, not explained** (below) |

### P1: the miss and the protocol change

The prediction assumed the heated case is isothermal "before boiling". It is
not: by 0.3 s the heater has started raising the liquid temperature in `mid`,
and the liquid density falls about 0.23 kg/m^3 per K, so the liquid expands
while it heats. An expansion rate `d(rho)/dt ~ 16 kg/(m^3 s)` over the 1 m
heated zone adds `~0.019 m/s` downstream, the order of the 0.0273 m/s
inlet-to-outlet difference measured. The closed form's premise holds only with
the heater off, so the check was re-run that way (the `fixedPower` models
switched off after reading, everything else upstream's): **2.25e-5**, inside
the 1e-3 bound. The heated 0.3 s state is kept as its own test pinning what was
measured (inlet within 3 % of the closed form, outlet faster than inlet). The
1e-3 bound was not moved; the protocol was changed so its premise holds.

### P2: the miss is in the prediction

The prediction took the heat-up rate from the structure alone (117 K/s). The
structure and the liquid it touches heat together: `(alpha_s rho Cp)_structure
+ (alpha rho Cp)_liquid = 1.925e6 + 0.553e6 J/(m^3 K)`, so 2.25e8 W/m^3 heats
them at **91 K/s**, which is the measured rate of the hottest liquid (739.8 K at
1 s, 831.5 K at 2 s, 922.9 K at 3 s: 91.6 K/s). From 652 K to the local
saturation temperature at the top of `mid` (~1136 K) is 484 K, i.e. 5.3 s at
91 K/s plus the time the flow takes to carry heat away; 6.14 s is consistent.
The port's number stands; the prediction's heat capacity was wrong.

### P5: not explained

The liquid's cumulative continuity error grows in two bursts: -0.0037 kg/m^3
by 6.0 s (single phase), then -0.76 kg/m^3 between 6.2 s and 6.5 s (boiling
onset) and -1.47 kg/m^3 between 9 s and 10 s (collapse). The vapour's stays
below 2e-3 kg/m^3. A likely mechanism, **not confirmed**: with
`solvePhase vapour` the liquid fraction is `1 - alpha_s - alpha_v`, so any error
in the vapour's volume balance is carried into the liquid's mass at the liquid
density (~860 kg/m^3, ~1600 times the vapour's). Two diagnostic ablations, not
defaults, move it but do not remove it: `alpha` relaxation 1 instead of 0.5
gives -1.017 kg/m^3; 100 outer correctors instead of 24 gives -6.958 kg/m^3.
Both also change the excursion (minimum inlet velocity -3.12 and -2.11 m/s
against -0.94 m/s), so **the boiling phase of this run is sensitive to its
iteration controls**: 426 of the 4577 steps ended at the 24-corrector limit
without meeting `residualControl`. Whether upstream (with MULES) behaves the
same is unknown, ~~since upstream cannot be run here~~ **ANSWERED 2026-11-12** (gh:#1361 round 3, real binary, same `implicitUpwind`): upstream's own
cumulative liquid continuity error is **-0.4466 kg/m^3 at the shipped solver tolerances and -2.2965 kg/m^3 at
1e-14**: it also moves by a factor 5 with the linear-solver tolerance alone, in the same direction as the port's
(then -2.675; -1.860 shipped and -4.290 tight after round 3). The -2.7 is a property of the case's boiling
phase, not a port-only defect. Not an upstream-parity number: the boiling phase amplifies a 1e-6 difference to
O(1) within 0.5 s.

## Plots

- `1d_boiling_void_profiles.png`: void fraction along the channel at 6.2 to
  10 s. Vapour first appears at the top of `mid` (x = 1.45-1.6 m), fills the
  top of the heated zone and the riser above it, and condenses after 7 s.
- `1d_boiling_temperature_profiles.png`: liquid, vapour, structure and
  interface (saturation) temperatures at 3, 7 and 9 s. At 7 s the liquid and
  the structure reach the saturation line from x ~ 1.3 m.
- `1d_boiling_history.png`: inlet liquid velocity, maximum void, maximum
  structure temperature against time; power off at 7 s.

**No upstream curve is overlaid on any plot, because upstream ships none for
this case ~~and cannot be run here~~** (**CORRECTED 2026-11-12**: it can be, and was; the comparison is in
`genfoam_vs_upstream_gh1361/README.md`, in numbers rather than overlays). The geometry is the 40-cell, 2 m channel
of upstream's `blockMeshDict`; the heated zone boundaries (0.5 m, 1.5 m) are
marked on the void plot and match the `cellZones` read (cells 10-29).

## What is not established

- ~~**Parity with upstream** on any two-phase quantity. Nothing here compares the
  port with an upstream run; the numbers above are the port's alone.~~ **CORRECTED 2026-11-12** (gh:#1361 round 3): this record compares nothing with
  upstream, but `genfoam_vs_upstream_gh1361/README.md` "Round 3" does (65 steps to 1e-9, the onset to 3e-6; pointwise
  agreement is lost within 0.5 s of the boiling onset).
- The other four feature cases: `1D_PSBT_SC` and `1D_CHF` need closures that
  are not ported (the case reader stops on them by name);
  `2D_voidMotionNoPhaseChange` and `2D_KNS37-L22` were not attempted.
- `phaseProperties.residualKd` (1D_boiling sets 10) is read by nothing in
  upstream `src/` at `652b3da` (grep finds no use); the port ignores it too.
