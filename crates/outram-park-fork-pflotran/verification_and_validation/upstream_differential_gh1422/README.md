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

### Addendum predictions, gold-file rows 3-5 (gh:#1537), written 2026-11-13 BEFORE any port run of a regression deck

Candid ordering note: the upstream source files below (`richards.F90`, `richards_common.F90`,
`richards_aux.F90`, `pm_richards.F90`, `timestepper_SNES.F90`, `timestepper_base.F90`,
`convergence.F90`, `solver.F90`, `patch.F90`, `characteristic_curves_common.F90` at 669fbce)
were READ before these were written, and the shipped `infiltration1d.regression.gold`
(20 steps, 62 Newton iterations) was known from round 1. No port number existed. The step count is
therefore a consistency check of my reading of the controller, not a blind prediction; the
pressures are blind. Nothing below was tuned to a port result.

Deck: `regression_tests/default/infiltration1d/infiltration1d.in` (40 x 1 x 1 cells, dx = 0.005 m,
horizontal, `SATURATION_FUNCTION LINEAR` with Sr 0.3333333, alpha 1e30, pcmax 9807 Pa,
`BURDINE_LINEAR_LIQ`, constant EOS 997.16 kg/m3 and 1e-3 Pa s, saturation 0.4444 initially,
saturation 1 on the west face, no-flow east, `TIME` final 9504 s, max dt 1000 s, output times 0 864 5184 9504 s).

| id | surface | prediction (fixed before running) |
|---|---|---|
| P17 | deck reader | parses the whole deck with no unsupported card: 40 x 1 x 1 cells, dx = 0.005 m, 4 regions, 2 flow conditions, regression cell ids 5 15 25 35 (1-based), output times 0 864 5184 9504 s |
| P18 | `SFLinear*` saturation, `RPFBurdineLinearLiq` kr, built from the Fortran source | only `+ - * /` (no `pow`/`exp`), identical operation order, so BIT-IDENTICAL on every point of a grid (pc from -10 to 2 pcmax; Sl from 0 to 1.2), checked against the verbatim Fortran compiled with gfortran `-O2 -mno-fma` |
| P19 | initial pressure from `LIQUID_SATURATION 0.4444` through `SFLinearCapillaryPressure` (`PatchUpdateCouplerSaturation`) | `p0 = 101325 - pc`, `pc = (1/alpha - pcmax) Se + pcmax`, `Se = (0.4444 - Sr)/(1 - Sr)`: `p0 = 93151.6 +- 0.1 Pa`; the gold minimum (93152.55 Pa) must lie ABOVE it, because the far cells have wetted slightly by 9504 s |
| P20 | time-step sequence | `TIMESTEPPER` defaults: first dt 1 s, growth capped at 2 (Newton < 5 iterations, `PMRichardsUpdateTimestep` with `ts_acceleration` 5), `dt_max` 1000 s, output times forced and dt reverted afterwards. Derived by hand: dt = 1,2,4,...,256 (9 steps, t = 511), 353 (to 864), 512, 1000, 1000, 1000, 808 (to 5184), 1000, 1000, 1000, 1000, 320 (to 9504): **20 steps, final time exactly 9504 s** |
| P21 | Newton iterations in total | **62**, the gold value, if the SNES test (`rtol` 1e-8 on the 2-norm of the residual, `stol` 1e-8, minimum one iteration, PETSc defaults because `newton_inf_*_tol` are UNINITIALIZED = -999 and `check_post_convergence` is off) is translated and the first full step satisfies the line-search decrease condition. Moderate confidence: the port uses a finite-difference Jacobian where upstream is analytic, and an exact tridiagonal solve where upstream does one BiCGStab/ILU iteration per Newton step (62 linear = 62 Newton in the gold). A different count is a FINDING to localise, not a number to tune |
| P22 | gold pressures at cells 5, 15, 25, 35, max, min, mean; saturations likewise | with the same dt sequence, both codes converge to the same fixed point of the same backward-Euler equations and differ only by the Newton stopping residual. A step stops when `||F|| <= 1e-8 ||F0||` (or the update is below 1e-8 of `||x||`), so the error left in a step is at most `1e-8 x` (Newton step size of that step, at most the pressure change across the step, below ~3e3 Pa) = 3e-5 Pa; diffusion damps the accumulation over 20 steps, bound 20 x 3e-5 < **1e-3 Pa absolute (1e-8 relative)** per pressure. Saturation is `0.6667/9807 = 6.8e-5 /Pa` times that: **<= 7e-8**. If the step sequence differs the bound does not apply (backward Euler is first order in dt; the difference could then be 1e-3 relative) |
| P23 | `Solution 2-Norm` (gold 6.0421481158758E+05 Pa) | same bound as P22 propagated: `<= 40^0.5 x 1e-3 = 6.4e-3` |
| P24 | `Residual 2-Norm`, `Linear Solver Iterations` | NOT compared. The residual norm at exit depends on the last Newton overshoot (gold 3.6e-14 in kmol/s; the port is in kg/s, a factor 18.01534 larger), and the linear iteration count is a property of PETSc BiCGStab |

Route, results and the lineage audit (`lineage.md`) follow the addendum.

### Addendum predictions, isotherms (written AFTER `isotherm_kernel` was first run, BEFORE any Rust comparison)

Candid ordering note: the Fortran kernel output `isotherm_output.txt` was
generated before these predictions were written down, but no port number had been
compared to it. They follow from reading the two formulas only.

| id | surface | prediction |
|---|---|---|
| P14 | linear `Kd*c` | bit-identical |
| P15 | Langmuir `s = (K c) b/(1+K c)` vs port `s_max (k c)/(1+k c)` with `s_max=b`, `k=Kd`; slope upstream `res/m - res/(1+t) t/m` vs port `s_max k/(1+k c)^2` | `s` bit-identical (the multiplications commute); slope algebraically equal with cancellation factor `(1+t)`, `t = Kd*c <= 12.5`: relative <= 1e-14 |
| P16 | Freundlich: the same number `n` given to both | NOT the same isotherm (upstream `c^(1/n)`, port `c^n`): relative difference large for every `n != 1`; with the port given `1/n` (new `new_freundlich_pflotran`): `s` relative <= 4.5e-16 and slope relative <= 1e-14 (libm `pow` only) |

## Route taken (gh:#1422 feasibility order)

1. **conda-forge `pflotran` package: does not exist** (`micromamba search "*pflotran*"` on conda-forge, 2026-11-13: no entries). Route 1 is unavailable.
2. **PETSc + PFLOTRAN source build: taken, fits.** conda-forge PETSc 3.24.6 (real, mpich, hdf5 2.2.0 mpi) with its gfortran 16.2.0 supplied PETSc; PFLOTRAN 669fbce (needs PETSc >= 3.24.3) was compiled from source: 332 Fortran files, `make -j2`, about 25 minutes, one link retry (the makefile needs `-lhdf5_hl_fortran -lhdf5_fortran`; the conda PETSc does not set `have_hdf5`). Disk: environment 1.5 GB after `micromamba clean -a` (2.6 GB before cleaning), clone 49 MB, objects a few hundred MB. The environment is deleted when finished.
3. Route 3 (verbatim transcription + grep check) was used only for the isotherm kernel, because `ReactionIsothermTotalSorbKD` needs a full `rt_auxvar`. `check_verbatim.sh`: 11 of 11 statements present byte for byte at lines 326-343.

**Faithfulness of the build.** `default/infiltration1d` was run with the built binary: `infiltration1d.regression` equals the shipped `.regression.gold` in every number (20 time steps, 62 Newton iterations, pressures 9.3152551889714E+04 to 1.0116103227878E+05); the only diff line is a wall-time line the gold file lacks (`infiltration1d_vs_gold.diff`). Gold criterion: 1e-9 absolute pressure.

Compiler flags of the upstream objects (conda): `-O3 -ftree-vectorize -march=nocona` (no FMA). The driver is `-O2 -fno-tree-vectorize`. Upstream `pow` is glibc; the port uses PETIR libm.

## Results (2026-11-13; counts are bit-identical / compared)

| id | surface | n | bit-identical | worst relative | prediction | outcome |
|---|---|---|---|---|---|---|
| P1 | VG `Sl(pc)`, exact `m` (sets 1, 2) | 195 | 195 | 0 | <= 4.5e-16 | held, no libm difference at all |
| P2 | VG `Sl(pc)`, inexact `m` (sets 3-5) | 377 | 299 | 1.18e-15 (set 4, pc 5.6e9) | <= 1e-14 | held |
| P3 | upstream `(alpha pc)^n < 1e-15` guard | 78 VG points | 78 after fix | before the fix the slope was ~1e-12 vs upstream 0 | Sl diff <= 1e-15 | partly missed: the Sl prediction held but I had not predicted the slope; defect D2 |
| P4 | `dSl/dP` VG exact / inexact / BC | 193 / 374 / 255 | 96 / 81 / 115 | 4.4e-16 / 6.4e-16 / 1.9e-15 | <= 1e-13 | held, except one point before the fix: BC at `pc = 1/alpha` had relative difference **1** (defect D1) |
| P5 | VG-Mualem `kr` | 505 | 288 | 2.68e-14 (set 5, Sl 0.439) | per point `8 eps (1+A)` | held |
| P6 | VG-Mualem `dkr/dSl` | 495 | 172 | 1.46e-14 | per point `64 eps (1+A)` + 1e-13 | held |
| P7 | BC `Sl` (260 compared + 390 saturated points exact); Burdine `kr` | 650 / 505 | 650 / 505 | 0 / 0 | <= 2e-15 | held |
| P4 | BC-Burdine `dkr/dSl` | 495 | 324 | 4.1e-16 | <= 1e-13 | held |
| P8 | water density IF97, Region 1 | 39 | 22 | 4.6e-16 | <= 1e-11 | held |
| P9 | enthalpy IF97 | 39 | 15 | 3.7e-16 | <= 1e-9 | held (J/kmol divided by 18.01534) |
| P10 | saturation pressure IF97 | 35 | 16 | 4.7e-15 | <= 1e-11 | held |
| P11 | IFC67 density vs `LiquidWaterEos::water()`, T <= 100 C, P <= 10 MPa | 14 | 0 | **4.37e-2** (T 100 C) | <= 1.5e-2 | **MISSED**: the port's EOS has no temperature dependence (1000 kg/m3 reference) while IFC67 water at 100 C is about 958 kg/m3. Model difference, not a defect; pinned in `p11_..._prediction_missed` |
| P12 | `EOSWaterViscosity1` vs IAPWS viscosity, 5-300 C | 37 | 0 | 4.5e-2 (T 300 C) | <= 5e-2 | held, narrowly |
| P13 | Debye-Huckel `A` | 1 | 0 | -5.67e-3 | -5.7e-3 | held; upstream 0.5114 / 0.3288 / 0.0410 vs port 0.5085 / 0.3281 |
| P14 | linear isotherm `s`, `ds/dc` | 18 | 18 | 0 | bit-identical | held |
| P15 | Langmuir `s` / slope | 54 | 54 / 18 | 0 / 1.14e-15 | `s` bit-identical, slope <= 1e-14 | held |
| P16 | Freundlich, same number | 54 | 24 (the `n = 1` cases) | **1.0** | different isotherms | held: convention inverted (D3) |
| P16 | Freundlich, PFLOTRAN parameterisation | 54 | 54 / 43 | 0 / 2.6e-16 | <= 4.5e-16 / 1e-14 | held |

Decay (gh:#1000, unchanged): 18/18 decay constants, 165/165 three-generation masses, 115/115 implicit masses bit-identical to the scalar upstream build. Re-checked 2026-11-13: both upstream decay files are byte-identical at 669fbce.

Accepted PETIR-libm differences: none in saturation values. Differences appear in derivatives, `kr` and EOS, always inside the bounds above; counts are in the table.

## outram-park-mpi #1435 (allreduce -> recursive doubling, long reduce -> reduce-scatter-gather)

`tests/mpi_sensitivity_gh1435.rs` runs `distributed_cg` (48 unknowns, shift 0.1, tol 1e-10) and a 1000-term `distributed_dot` on 1..8 ranks. Hash of the assembled solution bits, before = `outram-park-mpi` at `7fdc2b11e`, after = HEAD (2990e4737):

| ranks | CG iterations | solution hash | dot bits |
|---|---|---|---|
| 1, 2, 3, 4, 6, 7, 8 | 48 | identical before and after | identical |
| **5** | 48 | before `c6f7eee22c50e49d`, after `15659272a454fc6f` | identical |

**One number moved**: the distributed CG solution on 5 ranks changed at the bit level (iteration count and dot products unchanged). The magnitude was not measured (hash only); the distributed-versus-serial tests that bound it (1e-8) pass. The "after" column is pinned by the test.

## Gold-file results, `default/infiltration1d` (gh:#1537; run 2026-11-13, `tests/regression_decks_gh1537.rs`)

Method: `parse_regression_deck` reads the verbatim deck (`regression_decks/default/infiltration1d/`);
`run_regression_deck` builds the crate's `RichardsProblem` (constant EOS 997.16 kg/m3, 1e-3 Pa s, `Linear`
curves) and steps it with the translated PFLOTRAN time-step controller and SNES convergence test
(`src/flow/regression_run.rs`, sources and line numbers in its header). Reference: the shipped
`infiltration1d.regression.gold` (reproduced bit for bit by the round-1 upstream build). Bounds: P17-P24 above,
committed in `ac0228be4` before any run and unchanged.

| id | quantity | prediction | port | outcome |
|---|---|---|---|---|
| P17 | deck reader | reads the whole deck | 40 x 1 x 1, dx 0.005, 4 regions, 2 flow conditions, cells 5 15 25 35, times 0 864 5184 9504 | held |
| P18 | `Sl(pc)`, `dSl/dP`, `kr` chain, `pc(Sl)`; 3 parameter sets | bit-identical | **405 / 405** Sl, dSl, kr and **147 / 147** `pc(Sl)` bit-identical to the verbatim Fortran (39 / 39 statements byte for byte, `check_verbatim_linear.sh`); `dpc/dSl` and the direct `kr(Sl)` grid are not exposed by the port and were not compared | held |
| P19 | initial pressure | 93151.6 +- 0.1 Pa | 93151.8466 Pa | **MISSED by 0.25 Pa**: my hand arithmetic rounded `Se` to 0.1666 (exact 0.16660004167); the formula (`PatchUpdateCouplerSaturation` through `SFLinearCapillaryPressure`) was right, the closed form evaluated independently gives 93151.84660865767 and the port agrees. The ordering check (gold min above it) held |
| P20 | time steps | 20; dt = 1,2,4,...,256,353,512,1000,1000,1000,808,1000,1000,1000,1000,320 s | 20 steps, exactly that dt sequence, final time 9504 s, 0 cuts | held (a consistency check of my reading of the controller, as stated) |
| P21 | Newton iterations | 62 | **62** (per step 2,2,3,3,3,3,3,4,4,4,4,4,3,3,3,3,3,3,3,2; the gold file stores only the total) | held |
| P22 | pressure at cells 5 15 25 35, max, min, mean | <= 1e-3 Pa | worst **5.4e-9 Pa** (cell 15); the others 3e-10 to 2.0e-9 | held, by five to six orders |
| P22 | saturation, same quantities | <= 7e-8 | worst **3.5e-13** (cell 15) | held |
| P23 | `Solution 2-Norm` 6.0421481158758E+05 | <= 6.4e-3 | 6.0421481158758E+05, difference 2.9e-9 | held |
| P24 | residual 2-norm, linear iterations | not compared | not compared | - |

Per-quantity differences (port minus gold; Pa and dimensionless):

| quantity | port | gold | diff |
|---|---|---|---|
| P Max | 1.0116103227878e5 | 1.0116103227878e5 | -2.0e-9 |
| P Min | 9.3152551889714e4 | 9.3152551889714e4 | 2.9e-10 |
| P Mean | 9.5497616647453e4 | 9.5497616647454e4 | -5.7e-10 |
| P cell 5 | 9.9772708306190e4 | 9.9772708306191e4 | -1.1e-9 |
| P cell 15 | 9.5924158770541e4 | 9.5924158770536e4 | 5.4e-9 |
| P cell 25 | 9.3419001381497e4 | 9.3419001381498e4 | -1.4e-9 |
| P cell 35 | 9.3156150022515e4 | 9.3156150022514e4 | 8.3e-10 |
| S Max / Min / Mean | 9.8885369433939e-1 / 4.4444794405981e-1 / 6.0386229945170e-1 | ...942e-1 / ...981e-1 / ...171e-1 | -3.1e-14 / 3.4e-15 / -6.9e-15 |
| S cell 5 / 15 / 25 / 35 | 8.9447729367291e-1 / 6.3285806060293e-1 / 4.6256082296296e-1 / 4.4469254029926e-1 | ...295e-1 / ...258e-1 / ...303e-1 / ...924e-1 | -4.4e-14 / 3.5e-13 / -6.5e-14 / 2.5e-14 |
| Solution 2-Norm | 6.0421481158758e5 | 6.0421481158758e5 | 2.9e-9 |

**Interpretation.** The agreement is round-off level (relative 1e-14), far inside the bound derived from the
Newton stopping tolerance, so the derived bound was very conservative: the two codes do not merely land in the same
tolerance band, they take the same steps and the same Newton counts. It is NOT bit-identity: PFLOTRAN's own gold
criterion is 1e-9 Pa, and the port exceeds that in 5 of the 9 pressure quantities (up to 5.4e-9 Pa, i.e. 5 times the
criterion, cell 15). The remaining difference was not localised to a single operation: the candidates are the
finite-difference Jacobian and the exact (ILU(0) on a tridiagonal) solve against upstream's analytic Jacobian and one
BiCGStab iteration, and operation order in the flux (`Dq * ukvr * dphi * area * density` upstream). No ablation was run.
Do not quote this as "matches to 1e-9"; quote "matches to 5.4e-9 Pa absolute, 6e-14 relative, same 20 steps and 62 Newton iterations".

**Defects found (port, fixed by translating upstream).** None in the flow operator. Two missing pieces were added:
`SATURATION_FUNCTION LINEAR` / `BURDINE_LINEAR_LIQ` (`properties::Linear`) and the deck reader. One latent mismatch
fixed on the way: `Linear::d_se_d_pc` is the constant slope up to and including `pc = pcmax` (upstream truncates
`pc` at `pcmax`, it does not zero the slope there).

**Not translated, and what the run depends on.** (1) PETSc's backtracking line search: the runner accepts the full
step when PETSc's first test passes and returns an error otherwise; the test never hit that error, so backtracking
did not occur in this deck. (2) Waypoints at time 0 are dropped (upstream pop logic not read). (3) `ts_acceleration`,
`pressure_dampening_factor`, `saturation_change_limit`, `ITOL_*` options and `NUMERICAL_METHODS` blocks are not read;
decks that set them are refused.

### Rows not measured (gh:#1537 ledger)

| deck | why not run |
|---|---|
| `default/543/543_flow_eos_constant` | 3-D, 4 materials with anisotropic permeability and curves, hydrostatic initial condition, Neumann flux, `TS_ACCELERATION 8`, `KSP PREONLY / LU`: the port's `RichardsProblem` is homogeneous and isotropic with Dirichlet-only boundaries |
| `default/capillarity/disable_capillarity_richards` | `DISABLE_CAPILLARITY`, hydrostatic IC, Neumann flux, default IFC67 water density (not translated; P11 shows the port's EOS differs by 4.4 %), `EXTERNAL_FILE` |
| `default/steady/1d_steady` | `STEADY_STATE` option and IFC67 density (the gold pressures deviate 0.013 Pa from the linear profile because of it) |
| `default/column/*` (reactive transport) | global-implicit reactive transport against the port's operator-split `reactive_transport`; needs `CHEMISTRY` reading, `MINERAL_KINETICS`, `TRANSPORT_CONDITION` |

### Deferred decisions for the maintainer (gh:#1537)

1. Licence of the committed copies: the deck and gold file are LGPL-3.0 PFLOTRAN files; they are committed verbatim with
   `regression_decks/PROVENANCE.md`. If the maintainer prefers only extracted numbers, delete the two files and inline the constants.
2. Whether to translate PETSc's backtracking line search (needed before any deck that fails the first decrease test).
3. Whether to translate `EOSWaterDensityIFC67` / `EOSWaterViscosity1` (the PFLOTRAN default EOS) so that decks without an `EOS WATER` block can run: it unlocks `1d_steady` and `capillarity`.
4. Whether `RichardsProblem` should grow heterogeneous materials and anisotropic permeability (unlocks `543`).

## Gaps recorded

* `regression_tests/default/richards/`, named in gh:#1422 row 3, **does not exist** at 669fbce. The 1-D Richards decks are `infiltration1d`, `capillarity/*`; the 1-D reactive transport ones are `column/*`. Corrected on the issue.
* ~~The port cannot read any `regression_tests/default` deck (REGION, FLOW_CONDITION, BOUNDARY_CONDITION, STRATA unsupported) and has no `SATURATION_FUNCTION LINEAR` / `BURDINE_LINEAR_*`, which `infiltration1d` uses. Gold-file rows 3-5 of gh:#1422 therefore remain unmeasured; the built binary reproducing its gold is the groundwork. Child issue filed.~~ **CORRECTED 2026-11-13 (gh:#1537)**: the deck reader and the LINEAR/Burdine-linear curves exist and `infiltration1d` is compared against its gold (next section). Rows for `default/543` and `default/column` are still unmeasured.
* `reactive_transport` is operator-split; upstream's default is global implicit.
* Deferred decisions: see `lineage.md`.
