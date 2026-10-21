# Upstream PFLOTRAN decay: code-to-code record (gh:#1000)

> **Research, education and V&V only.** Not for nuclear facility operation,
> licensing, safety-critical or safeguards use (see `RESPONSIBLE_USE.md`).
> AI-generated and not yet human-reviewed.

This folder verifies the three upstream PFLOTRAN decay algorithms ported into
`src/decay/upstream.rs` as `DecaySolver::UfdThreeGeneration`,
`DecaySolver::UfdImplicit` and `DecaySolver::BackwardEuler`. The default solver
stays PETIR CRAM48 (maintainer decision on gh:#999).

## Why the routines were extracted rather than PFLOTRAN run

A full PFLOTRAN build needs PETSc. On the machine this was done on
(2026-10-20) gfortran 16.2.1 and `mpif90` are installed, but PETSc is not, and
there is no existing PFLOTRAN build. The UFD_DECAY decay solve is
self-contained, so its lines were cut out verbatim and compiled with a
minimal driver instead.

## Files

| file | what it is |
|---|---|
| `extract.sh` | cuts `pm_ufd_decay.F90` lines 1868-1994 and `utility.F90` lines 542-699 out of the upstream clone at commit `2af572b8759c34aacc2216f56a906637f90af03e`, after checking both files' SHA-256 |
| `ufd_body_extract.F90` | the solver body, verbatim; only `this%` deleted |
| `lu_extract.F90` | `LUDecomposition2/1` and `LUBackSubstitution`, verbatim; PETSc kinds replaced by intrinsic types, and the three MPI calls on the abort path deleted (the `print` and `stop` are kept) |
| `driver.F90` | hand-written: declarations, the parent list built as `PMUFDDecaySetup` builds it (lines 1203-1222), decay rates from half-lives as the `HALF_LIFE` card computes them (line 480), and the cases |
| `run.sh` | builds the driver twice and writes both outputs |
| `upstream_output.txt` | `gfortran -O2 -fno-tree-vectorize`: every `exp` is a scalar glibc call |
| `upstream_output_O2_vector.txt` | plain `gfortran -O2`: the array statement `solution = solution*exp(-rhs)` (line 1989) becomes glibc libmvec's `_ZGVbN2v_exp` |

Output lines are `<case> <E|I> <step> <isotope> <hex bits of the mass>`,
`<case> N <step> 0 <Newton iterations>`, and `L <case> <isotope> <hex bits of
the decay rate>`.

## Cases

| case | chain | start | steps |
|---|---|---|---|
| `radon` | Rn-222 to Po-218 to Pb-214 to Bi-214 to Pb-210 (5 members, unit branching) | 1000, 1, 1, 1, 1 | 10 of 1 h |
| `radon0` | the same | 1000, 0, 0, 0, 0 | 10 of 1 h, 3-generation only |
| `thc` | Pb-212 to Bi-212 to {Po-212 0.6406, Tl-208 0.3594} to Pb-208 | 1000, 1, 1, 1, 1 | 10 of 600 s |
| `conv` | P1 (4 s) and P2 (9 s) to D (stable) | 10, 20, 1 | 5 of 1 s |

The exact half-lives are in `driver.F90`.

## Method and pass criteria

The Rust tests in `src/decay/upstream_tests.rs` march the same cases and
compare every mass after every step, as bit patterns. Criteria, fixed before
the first run: bit equality for both paths against `upstream_output.txt`,
including every Newton iteration count. Against the vectorised build, fixed
after the two Fortran builds had been diffed (they differ in 19 of 115
implicit masses): 115 masses compared, worst relative difference below
`1e-13`, iteration counts equal.

## Results (2026-10-20, gfortran 16.2.1, glibc, x86-64)

| path | vs `upstream_output.txt` | vs `upstream_output_O2_vector.txt` |
|---|---|---|
| decay constants | 18 / 18 bit-identical | 18 / 18 |
| 3-generation masses | 165 / 165 bit-identical | 165 / 165 |
| `IMPLICIT_SOLUTION` masses | 115 / 115 bit-identical | 96 / 115; worst 2.4e-16 relative (1 ulp) |
| `IMPLICIT_SOLUTION` iteration counts | 25 / 25 (4 to 25) | 25 / 25 |

The first run of the implicit test was against the plain `-O2` build and
failed by 1 ulp (radon, step 5, isotope 4). Per-iteration bit dumps from both
sides showed iteration 1 identical and the first difference in
`solution*exp(-rhs)`; `nm` showed `_ZGVbN2v_exp` in the binary. Building with
`-fno-tree-vectorize` removed it. Which variant a real PFLOTRAN build matches
depends on its PETSc `FOPTFLAGS`.

## Upstream behaviour found while doing this

- **Zero daughter inventory, `IMPLICIT_SOLUTION`:** `driver zero` (A to B, B
  starting at exactly 0) never returns. A debug build printed the iterate
  until it was killed past 1.9e6 iterations; the same values recurred
  1.1e6 iterations apart, so it cycles. Upstream has no
  iteration cap. The port stops at 1000 iterations with an error.
- **Zero parentless inventory, `IMPLICIT_SOLUTION`:** `driver zeroparent`
  stops in `LUDecomposition` with "Singular value encountered" (upstream calls
  `MPI_Abort`). The port returns an error.
- **The 3-generation path never reads the branching fractions** (lines
  1880-1925). A daughter with branching 0.6 grows as if it were 1.
- **An inventory of all ones is returned undecayed by `IMPLICIT_SOLUTION`**,
  because `prev_solution` starts at 1 (line 1931) and the convergence test
  passes before the first iteration.

Analytic checks against CRAM48 (exactness at depth 3, second-order
truncation of the 3-generation form, first-order backward Euler, atom
balance, implicit Newton vs direct backward Euler) are recorded with their
predictions and measured numbers in the test doc comments.

## Provenance

PFLOTRAN, https://bitbucket.org/pflotran/pflotran, commit `2af572b8`
(2026-10-20), LGPL-3.0, used under GPL-3.0 within this crate's AGPL-3.0
combined work; see the crate `NOTICE` and `upstream_source/README.md`.
