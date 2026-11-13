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

(Results and corrections are appended below.)
