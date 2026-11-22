# Predictions, written BEFORE any comparison run (gh:#1407, 2026-11-13)

Committed ahead of the first comparison so it can fail. Upstream: Thermochimica
`ORNL-CEES/thermochimica` commit `0c35c8d7d1cf2084b4e2ca5d6608f7dcdf60adad` (BSD-3-Clause),
cloned over git into an empty scratch directory, built with `make` (gfortran 13-class,
`-O2 -fbounds-check -ffpe-trap=zero`, system LAPACK/BLAS), never modified.
`tc_driver.F90` (this directory) is linked against the unmodified `libthermochimica.a`.

**Disclosure of timing.** Two things were observed before this file was written. (1) The
build links with the upstream Makefile as shipped. (2) One smoke run of the driver on case
C1 (CO.dat, 1000 K, 1 atm, C 2 mol, O 1 mol) to learn the output conventions: upstream
reports graphite + `gas_ideal` (CO 0.547 mol, CO2 0.227 mol), 3 global iterations, `g0rt`
and `mu` dimensionless, pure-condensed `mu` printed as 0, and the last two "pure condensed"
species of `CO.dat` (names ending `#`) are dummy species with zero Gibbs energy. Nothing was
run on the Rust side. Everything else below is a prediction.

What I read before predicting (source, not a run): `gem/CompExcessGibbsEnergyRKMP.f90`
(binary branch, `iParam` type 2; for the species outside the pair
`mu_k = -(1+e) x1 x2 L dx^e`, which equals the multicomponent partial-molar relation this
crate implements), `parser/ParseCSDataBlockRKMP.f90` (`2 / i j k` then `k` lines, exponent
`j-1`, so `1 2 1` is the *regular* term), `setup/InitThermo.f90` (`R = 8.314472`),
`module/ModuleThermoTolerance.f90` (default mass-balance and function-norm tolerance
`dTolerance(1) = 1e-5`; `SetToleranceOverrideValue` clamps at `1e-14`), and this crate's
`gem.rs` (`R = 8.31446261815324`, default `tol = 1e-10`).

## What is compared, and how the inputs are shared

The port has no ChemSage parser and no Gibbs-polynomial evaluator (epic #1408), so the
data-free minimiser is fed what upstream itself evaluated: for each species the
dimensionless `g0/RT` (`dStdGibbsEnergy`), the atom matrix (`dStoichSpecies`), the phase
membership, and the RKMP parameters (`dExcessGibbsParam`). That isolates the **minimiser,
the ideal-gas / ideal-solution terms and the RKMP excess term**. It does not test the
polynomial evaluation or the parser (a gap, not a pass). Two input modes:

- **A (primary)**: `g0_J = (g0/RT)_upstream * R_port * T`, `L_J = (L/RT)_upstream * R_port * T`,
  i.e. the same dimensionless problem. Removes the `R` constant difference.
- **B (literal)**: the J/mol values upstream's `R` implies, `g0_J = (g0/RT) * R_up * T`, fed
  to the port with its own `R`. That is what a user would do; it shows the `R` effect.

Upstream is run at its default tolerance (`1e-5`) and tightened (`1e-10`, via the shipped
`SetToleranceOverrideValue`; `dTolerance(2)` and `(4)` stay at `1e-5`).

## Cases

Databases are the ones shipped in the upstream repository's `data/` (the repository is
BSD-3-Clause; `data/Disclaimer.txt` says some files were modified for software testing and
warns against other uses). Used only for this code-to-code comparison.

| id | database | T [K] | P [atm] | feed [mol] | origin of inputs |
|---|---|---|---|---|---|
| C1 | `CO.dat` | 1000 | 1 | C 2, O 1 | `test/Thermo.F90` shape |
| C2 | `CO.dat` | 300 | 1 | C 1, O 1 | `test/daily/TestThermo10-13` shape |
| C3 | `CO.dat` | 2500 | 1 | C 1, O 1 | mine (all-gas) |
| C4 | `CO.dat` | 1500 | 10 | C 3, O 2 | mine (pressure term) |
| H1 | `HO.dat` | 300 | 1 | H 2, O 1 | mine |
| H2 | `HO.dat` | 1000 | 1 | H 2, O 1 | mine |
| H3 | `HO.dat` | 3000 | 1 | H 2, O 1 | mine (dissociation) |
| H4 | `HO.dat` | 2000 | 0.01 | H 4, O 1 | mine (pressure, H excess) |
| W1 | `WAuArO-1.dat` | 1455 | 1 | W 1.95, Au 1, Ar 2, O 10 | `TestThermo30` |
| W2 | `WAuArO-2.dat` | 1000 | 1 | W 1, Au 3, Ar 5, O 2 | `TestThermo31` |
| W3 | `WAuArNeO-1.dat` | 2452 | 1 | W 1.95, Au 1, Ar 2, O 10, Ne 10 | `TestThermo32` |
| W4 | `WAuArNeO-2.dat` | 900 | 2 | W 20, Au 2, Ar 7, O 5, Ne 1 | `TestThermo33` (two RKMP phases, miscibility gap) |
| S1 | `CsI-Pham.dat` | 400 | 1e-5 | Cs 1, I 1 | `TestThermo90` (ideal gas + RKMP; if it holds a phase type the port lacks it is recorded as out of reach) |

## Predictions

| # | claim | predicted outcome |
|---|---|---|
| P1 | phase assemblage in C1-C4, H1-H4 (ideal gas + pure condensed) | identical in all 8: same pure-condensed species present, gas present or absent alike. Pure-condensed phases below 1e-9 mol are counted as absent on both sides |
| P2 | upstream at default tolerance vs port, mode A, ideal cases | worst relative difference: element potentials <= 1e-5, species moles >= 1e-3 mol <= 1e-4, `G/RT` <= 1e-8 (stationarity makes `G` second order in the composition error). Upstream's own 1e-5 residual, not the port, sets these bounds. **The issue's "< 1e-8" prediction is expected to FAIL at upstream's default tolerance** because upstream stops at 1e-5 |
| P3 | upstream tightened to 1e-10 vs port, mode A, ideal cases | element potentials, moles >= 1e-3 mol, `G/RT` all <= 1e-8 relative, if upstream converges at that tolerance |
| P4 | bit-identical values | effectively none among solved quantities (different algorithm, different iterate path); only quantities fixed by the feed (e.g. element totals) can match. Counted and reported, not pinned |
| P5 | W1, W2 (one RKMP phase, 4 species, 4 elements: amounts fixed by mass balance) | species moles equal the feed to <= 1e-5 (default) on both sides; element potentials <= 1e-5; this checks the RKMP excess term through `mu` |
| P6 | mode B vs upstream (literal J/mol, different `R`) | systematic offset from `R_up/R_port - 1 = 1.1e-6`, so element potentials and `G` differ at ~1e-6, i.e. a real, constant-cause difference, not noise. Recorded as a finding (the crate's `R` is CODATA 2018, upstream's is 8.314472) |
| P7 | W4 (two RKMP phases with a gap) | the port, which has no miscibility-gap detection (upstream `CheckMiscibilityGap`), reaches the upstream two-phase assemblage only if the feed seeds the two phases asymmetrically; an even or one-phase seed converges to a one-phase stationary point of higher `G`. A gap, not a defect |
| P8 | iteration counts | no relation. Upstream: single-digit to tens (it starts from a leveling solution), port: tens to hundreds on the RKMP cases (frozen-gamma step). Reported, no bound |
| P9 | chemical-potential consistency | the port satisfies `mu_s/RT = sum_k a_ks pi_k` to its tolerance by construction; upstream to ~1e-5. Compared by the element potentials, not separately |

Any observed number outside these is reported as-is with its localisation. None of the
bounds above will be edited after the run.
