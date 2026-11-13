# Thermochimica 0c35c8d differential (gh:#1407, epic #1336, gaps on #1408)

Compares `outram-park-fork-thermochimica` against a **run of the upstream Fortran**.
`PREDICTIONS.md` was committed before the first comparison; `RESULTS.md` holds the
numbers, the defect found and fixed, and the gap survey.

## Upstream and how it was run

- Thermochimica, `https://github.com/ORNL-CEES/thermochimica`, commit
  `0c35c8d7d1cf2084b4e2ca5d6608f7dcdf60adad` (BSD-3-Clause), cloned over git into an empty
  scratch directory, built with `make -j2 all` (gfortran, `-O2 -fbounds-check
  -ffpe-trap=zero`, system LAPACK/BLAS, upstream Makefile unmodified).
- `tc_driver.F90` (this directory) is linked against the unmodified `libthermochimica.a`
  (`gfortran ... tc_driver.F90 obj/libthermochimica.a -llapack -lblas`). Per case it dumps the
  converged state and the data a data-free solver needs. `gen_inputs.py` writes its stdin.
- Two runs of the 15 cases: upstream default (`dTolerance(1) = 1e-5`, entry `Thermochimica`)
  and tightened to `1e-10` (`SetToleranceOverrideValue`, entry `ThermochimicaAPI`; the plain
  `Thermochimica` entry calls `InitThermo`, which resets the tolerances and does not reapply an
  override, so the tightened run has to go through the shipped coupling entry).
- Output: `thermochimica_reference.txt.gz` (both runs, `#mode default` / `#mode tight`).

## What is compared

The crate has no ChemSage parser and no Gibbs-polynomial evaluator, so it is fed what upstream
evaluated: dimensionless `g0/RT` (`dStdGibbsEnergy`), the atom matrix, phase membership and the
RKMP parameters (`dExcessGibbsParam`). That tests the **minimiser, the ideal-gas / ideal-solution
terms and the binary RKMP excess**; it does not test the parser or the polynomial evaluation.

- Mode A: `g0_J = (g0/RT)_up * R_port * T` (same dimensionless problem; `R` removed).
- Mode B: `g0_J = (g0/RT)_up * R_up * T` (literal J/mol, shows the `R` constant difference).
- "free": all candidate phases of the database (data-only reduction: of several pure condensed
  phases with identical atom vectors, e.g. graphite/diamond, the lowest-`g0` one is kept).
- "given": candidates restricted to the upstream-converged assemblage. The port has no
  assemblage selection, so this separates "equilibrium given the phases" from "which phases".
- The upstream gas-phase `ln P + 0.10945/R` (`setup/CompThermoData.f90:454-460`) is stored in
  `g0`; the `ln P` part is removed before feeding so the port's own `ln(P/P0)` is exercised.
- Port option ladder (fixed, applied to every case, rung reported): 0 `GemOptions::default()`;
  1 `tol = 1e-8`; 2 `mole_floor = phase_floor = 1e-50`; 3 both.

## Cases

13 from `PREDICTIONS.md` plus S2 and S3 added after the predictions were committed (same
database as S1, same prediction family P1-P3). `data/` files are the ones shipped in the upstream
repository; its `data/Disclaimer.txt` says some were modified for software testing and warns
against other use. They are used here only for a code-to-code comparison.

Run: `TC_DIFF_PRINT=1 cargo test --release -p outram-park-fork-thermochimica --test
thermochimica_differential -- --nocapture --test-threads=1`.

To regenerate the reference: build upstream as above, `python3 -I gen_inputs.py <clone>/data
0 > in_default.txt` (and `1e-10` for the tight run), run `tc_driver < in_*.txt`, and gzip the two
outputs behind the two `#mode` lines.
