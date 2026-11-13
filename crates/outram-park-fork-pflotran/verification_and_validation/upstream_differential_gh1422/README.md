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

## Gaps recorded

* `regression_tests/default/richards/`, named in gh:#1422 row 3, **does not exist** at 669fbce. The 1-D Richards decks are `infiltration1d`, `capillarity/*`; the 1-D reactive transport ones are `column/*`. Corrected on the issue.
* The port cannot read any `regression_tests/default` deck (REGION, FLOW_CONDITION, BOUNDARY_CONDITION, STRATA unsupported) and has no `SATURATION_FUNCTION LINEAR` / `BURDINE_LINEAR_*`, which `infiltration1d` uses. Gold-file rows 3-5 of gh:#1422 therefore remain unmeasured; the built binary reproducing its gold is the groundwork. Child issue filed.
* `reactive_transport` is operator-split; upstream's default is global implicit.
* Deferred decisions: see `lineage.md`.
