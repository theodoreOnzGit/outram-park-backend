# Linear solvers against OpenFOAM-v2506 (ESI), gh:#1362 row 3

Upstream line: ESI v2506 only (`libOpenFOAM` solvers, `libfiniteVolume`, unmodified).
Foundation (dev) was not read or run.

## Method

`scripts/run.sh` runs the REAL ESI `pimpleFoam` (gh:#1363 build) on a 20x20x1 lid-driven
cavity (`nu` 0.01, lid 1 m/s, dt 0.005, one time step, one PIMPLE corrector, orthogonal
mesh) with `solver dumpSolver;` for p (`driver/dumpSolver`, loaded with `libs`). At the
first (and only) pressure solve the dumpSolver prints the assembled matrix (diag, upper,
lower, addressing), the source and the incoming psi, then runs the real upstream solvers
`PCG`/`DIC`, `PBiCGStab`/`DILU`, `smoothSolver`/`GaussSeidel` (nSweeps 1) and `GAMG`/
`GaussSeidel` on that matrix for n = 1..30 iterations each (`tolerance 0`, `relTol 0`,
`minIter = maxIter = n`) from the incoming psi, printing the normalised initial and final
residuals and the iterate psi after n iterations (hex). The Rust test builds the matrix
from the dump and runs the port's solvers for the same n, comparing the iterate bit for bit
and the OpenFOAM-normalised residual of the port's iterate
(`LduMatrix::openfoam_initial_residual`).

## Predictions (written before the first run and before the Rust side exists)

1. The initial residual (`openfoam_initial_residual` of psi0) is bit-identical (gh:#1363).
2. **PCG/DIC**: the port's `conjugate_gradient` follows upstream's PCG recurrence; the
   iterate is bit-identical at n = 1 and stays within 1e-12 relative at n <= 30, with bit
   differences from the DIC grouping (`rD*upper*wA` association) and the `rD` reciprocal.
   Not predicted bit-identical for n up to 30.
3. **smoothSolver/GaussSeidel**: the port multiplies by `1/diag` where upstream divides by
   `diag`: not bit-identical from the first sweep, ~1e-16 relative per sweep, 1e-14 at n = 30.
4. **PBiCGStab/DILU**: the port has no DILU preconditioner (`ILU0` only, which also updates
   the off-diagonals): the iterate differs structurally (>1e-6 relative) from n = 1. A new
   function `pbicgstab_dilu` written from the upstream `PBiCGStab.C` and `DILUPreconditioner.C`
   is predicted bit-identical or within 1e-13 for n <= 30.
5. **GAMG**: the port's agglomeration is algebraic pairwise on `|upper|` with a symmetric
   V-cycle (2 pre and 2 post Gauss-Seidel sweeps) against upstream's `faceAreaPair`
   agglomeration with its own cycle: the agglomerations do not match, so no bit comparison is
   attempted; upstream's history is recorded and the difference in structure stated.

## Results (2026-11-13, release; 400-cell symmetric pressure matrix, first step)

Test: `tests/openfoam_solvers_code_to_code.rs`. Each solver: 30 iterates x 400 cells = 12000
values per row, compared bit for bit to the real upstream iterate after n iterations,
n = 1..30 (tolerance 0, so every solver runs exactly n iterations).

| solver | port function | bit-identical | worst relative | note |
|---|---|---|---|---|
| initial residual | `LduMatrix::openfoam_initial_residual` | identical (1.0 = 1.0) | 0 | gh:#1363 |
| PCG / DIC | `conjugate_gradient` | **12000/12000** | 0 | no change needed |
| smoothSolver / GaussSeidel (1 sweep) | `gauss_seidel` | 4737/12000 | 3.2e-13 | first differing iterate: 1 |
| smoothSolver / GaussSeidel | new `gauss_seidel_upstream` | **12000/12000** | 0 | `psii /= diag` and upstream `bPrime` order |
| PBiCGStab / DILU | new `pbicgstab_dilu` | **12000/12000** | 0 | written from `PBiCGStab.C`, `DILUPreconditioner.C` |
| PBiCGStab / DILU | existing `krylov::bicgstab` + `Preconditioner::ilu0` | 132/12000 | 2.0e2 | structural: ILU(0) is not DILU, differs from iterate 1 |
| GAMG | not compared | n/a | n/a | agglomerations differ, see below |

Upstream residual histories (normalised, as OpenFOAM reports them; the initial residual of
the first step is 1; recorded in `reference/ref_cavity_p.txt.gz`):

| n | PCG/DIC final | PBiCGStab/DILU final | smoothSolver/GS final | GAMG final |
|---|---|---|---|---|
| 1 | 1.40e0 | 9.86e-1 | 9.997e-1 | 5.31e-1 |
| 5 | 1.03e0 | 2.51e-1 | 9.973e-1 | 1.54e-3 |
| 10 | 1.98e-1 | 1.93e-2 | 9.877e-1 | 2.83e-5 |
| 20 | 1.65e-1 | 2.61e-3 | 9.272e-1 | 1.20e-8 |
| 30 | 7.38e-5 | 1.56e-9 | 8.340e-1 | 5.07e-12 |

(PCG's residual is not monotone; the matrix is a pure-Neumann Poisson problem with the
reference cell, which is why Gauss-Seidel barely moves it in 30 sweeps.)

**GAMG: recorded, not compared.** The port's `gamg` is an algebraic pairwise agglomeration
on `|upper|` with a symmetric V-cycle (2 pre and 2 post Gauss-Seidel sweeps, dense LU on
the coarsest level) and a 2-norm relative residual; upstream's default is `faceAreaPair`
agglomeration (mesh face areas), `nPreSweeps 0`, `nPostSweeps 2`, `nFinestSweeps 2`,
`scaleCorrection` on, with its own coarsest-level solver. The coarse grids are not the same
and no option to select upstream's agglomeration exists in the port, so a bit comparison
is not meaningful and was not attempted; the upstream history above is the reference a
future `faceAreaPair` port must hit.

### Predictions: held and not

- Held: 1 (initial residual identical), 3 (Gauss-Seidel not bit-identical from the first
  sweep: first differing iterate is 1; but the worst is 3.2e-13, so the predicted 1e-14 at
  n = 30 was too small by a factor of about 30), 4 (ILU(0)
  differs structurally from iterate 1; the new DILU function is bit-identical, better than
  the predicted 1e-13), 5.
- Wrong: 2. PCG/DIC was predicted NOT bit-identical for n up to 30 (DIC grouping and `rD`
  reciprocal); it is bit-identical for all 30 iterates: the port's DIC association
  (`rD*upper*wA`, `upper*upper/rD`) and its `x`, `r`, `p` recurrences are upstream's, and its
  guards (`1e-300`) never trigger here.

### Options and defects

- New (defaults unchanged): `pbicgstab_dilu`, `gauss_seidel_upstream`. No existing solver
  changed. The port's `krylov::bicgstab` remains ILU(0)/2-norm by design (Newton-Krylov
  use); it is not upstream's `PBiCGStab`.
- Port differences, not fixed: `gauss_seidel` uses `x = sigma*(1/diag)` (3e-13 after 30
  sweeps) and stops on a 2-norm criterion rather than upstream's L1 normalised residual;
  `conjugate_gradient` converges on `||r||_2/||b||_2`, not upstream's normalised L1 residual,
  so identical `tolerance` values stop at different iterations (not measured here).
- Moved numbers: none. No solver default changed; consumers still call `gauss_seidel` /
  `conjugate_gradient` / `krylov::bicgstab` as before. Switching consumers to the upstream
  forms would change iterates at the 1e-13 level (Gauss-Seidel) and structurally
  (BiCGStab preconditioner); not measured.

### Not covered

Coupled (cyclic/AMI) interfaces, asymmetric matrices (the cavity pressure matrix is
symmetric; PBiCGStab was run on an asymmetric-flagged copy with `lower = upper`), `smoothSolver`
with `symGaussSeidel`/`DIC`/`DILU` smoothers, `PBiCG`, `GAMG` (above), `nSweeps > 1`, the
solvers' stopping logic (`tolerance`/`relTol`/`minIter` paths): the runs force exactly n
iterations. Foundation (dev) was not read or run.
