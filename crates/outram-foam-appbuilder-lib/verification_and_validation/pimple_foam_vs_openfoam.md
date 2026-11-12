# `pimple_foam` vs upstream icoFoam, lid-driven cavity (gh:#1363, row 1)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

## Which upstream
The port's doc comments cite the ESI path `applications/solvers/incompressible/pimpleFoam`
but its ddtCorr and PISO text were translated from the Foundation line
(`fvc::ddt_corr` cites OpenFOAM-dev `EulerDdtScheme::fvcDdtPhiCorr`). The upstream
run here is therefore **OpenFOAM-dev 852527377c7e (Foundation)
`applications/legacy/incompressible/icoFoam`**, the real binary, built against the
already-built finiteVolume/meshTools (`scripts/run_cavity.sh`). pimpleFoam laminar
with `nOuterCorrectors 1` is icoFoam (the port's own statement). Dev's pimpleFoam
(`incompressibleFluid` module via foamRun) needs the momentumTransportModels libraries
and is not built (disk). The case is the dev tutorial `legacy/incompressible/icoFoam/cavity/cavity`
(1 m x 1 m x 0.1 m, 20 x 20 x 1, nu = 0.01, dt = 0.005, 100 steps to t = 0.5, Euler,
`Gauss linear` div(phi,U), `Gauss linear orthogonal` laplacian, PISO 2 correctors, no
non-orthogonal corrector, pRefCell 0 / pRefValue 0) with ONLY the linear-solver
settings tightened: p PCG/DIC, U smoothSolver symGaussSeidel, tolerance 1e-14,
relTol 0, maxIter 10000 (upstream converged in <= 50 iterations p, ~60 U every
step; log committed). Mesh: dev `blockMesh` output, committed under
`upstream_openfoam_gh1363/cavity/constant/polyMesh`, read by the port unchanged.

## Method
`tests/upstream_cavity_gh1363.rs` reads the committed polyMesh, the 0/ fields and
the committed reference (U, p per cell at steps 1, 2, 5, 10, 25, 50, 100 written at
17 significant digits, which round-trips; initial residuals for all 100 steps),
runs `PimpleFoam` with `Gauss linear` convection, Euler ddt, orthogonal Laplacian,
2 correctors, and counts bit-identical cells and the worst relative difference
(normalised by the field's max |value| at that step, and per cell) for Ux, Uy, p.
Uz is compared as an absolute magnitude (empty direction: upstream does not solve it).
Initial residuals: upstream's `lduMatrix::solver::normFactor` definition is
reimplemented on the port's matrix (`sum|b - Ax| / (sum|Ax - xavg*sumA| + |b - xavg*sumA|)`).

## Predictions (written 2026-11-11, BEFORE the port was run)
Read: dev `icoFoam.C`, `EulerDdtScheme::fvcDdtPhiCorr`, `ddtScheme::fvcDdtPhiCoeff`
(flux-normalised coupling coefficient, zero on non-coupled boundaries; the port's
`fvc::ddt_corr` has the same formula), and the port's `step_flow`.
- The algorithm is the same (momentum predictor with -grad p, `HbyA`, `phiHbyA =
  flux(HbyA) + rAUf*ddtCorr`, constrainHbyA, Laplacian pEqn, `phi = phiHbyA - flux`,
  `U = HbyA - rAU grad p`). I expect NO structural defect on this case.
- **Bit-identity is not expected for p or U, even at step 1.** The linear solvers
  differ (port: CG with its own preconditioner and a Gauss-Seidel whose sweep order
  is not stated to be symmetric; upstream: DIC-PCG and symGaussSeidel), so the
  converged solutions differ at the level of the 1e-14 stopping tolerance times the
  condition number, plus assembly-order rounding (`rAU = V/A` against `1/(D/V)`,
  `h*(1/a)`). Prediction: 0 of 400 cells bit-identical in Ux, Uy, p (exact zeros apart).
- **Step 1:** worst relative difference (against field max) <= 1e-10 in U and p; I
  expect 1e-13 to 1e-11.
- **Step 100 (t = 0.5):** <= 1e-9 (the flow is dissipative, so differences are not
  amplified; I expect growth of at most one decade over the run).
- **Initial residuals:** the first-step Ux = 1 and p(corrector 1) = 1 should match to
  1e-12 relative; later steps agree to 1e-8 relative or better on residuals that are
  themselves >= 1e-6; once an upstream residual is < 1e-9 it is round-off noise and
  is not compared. A mismatch beyond that would point to a different normalisation
  or a different assembly.
- **Uz:** exactly zero in the port; upstream prints O(1e-20) noise in the empty direction.
- If the agreement is looser than 1e-8 the first suspect is the boundary treatment of
  the moving lid in `fvm::div` (Gauss linear on the lid face), then `ddtCorr` on
  boundary faces.

## Results (measured 2026-11-11, release; `tests/upstream_cavity_gh1363.rs`)

**Process note.** The committed reference was regenerated once after the first
port run: the first extraction held U and p only; after the first port run
showed a step-2 disagreement, `phi` (face flux) was added to the reference to
locate it. Nothing in the upstream run changed (same binary, same inputs).

### First run, as the code stood before this work: the prediction MISSED
| | step 1 | step 2 | step 100 |
|---|---|---|---|
| Ux, Uy, p worst difference / field max | 7e-15, 1e-14, 4e-14 (as predicted) | **1.4e-3, 3.0e-3, 5.0e-3** | **1.4e-3, 2.8e-3, 3.5e-3** |

Step 1 agreed to ~1e-14, step 2 did not, and the initial Ux residual of step 2
differed (0.14805 against upstream 0.14858) before the pressure solve, so the
momentum matrix differed. `phi` agreed after step 1 (6e-15), so the difference was
in the operator, not the state: **defect D1**, below. The prediction "<= 1e-9 at
t = 0.5" was wrong for the code as it stood, and right after the fix.

### After the fixes (D1, D2): what the test pins
Worst `|port - upstream| / max|upstream field|` over the 7 snapshots (steps 1, 2, 5,
10, 25, 50, 100), cell by cell:

| quantity | step 1 | step 2 | step 100 | worst, any step |
|---|---|---|---|---|
| Ux | 3.0e-15 | 2.0e-14 | 2.5e-14 | 6.0e-14 |
| Uy | 8.9e-15 | 4.2e-14 | 5.0e-14 | 1.3e-13 |
| p | 2.5e-14 | 5.7e-14 | 4.5e-14 | 1.1e-13 |
| phi (760 faces) | 4.2e-15 | 1.3e-14 | 2.5e-14 | 9.4e-14 |

Bit-identical cells: 0 to 5 of 400 per field and step (coincidence, as predicted
that none would be). Worst per-cell relative difference on cells above 1e-6 of
the field max: Ux 4.7e-9, Uy 2.6e-10, p 2.2e-9 at their worst steps (cells where
the field is small). |Uz| of the port: <= 1.4e-15 (predicted exactly zero: wrong,
it is round-off from `Sf_z` of the empty faces; upstream prints ~1e-20).
**The test asserts worst < 3e-13 of field max** (measured 1.3e-13) and fewer than 10 %
of cells bit-identical. The bound is not relaxed.

Initial residuals (upstream normalisation, 100 steps x Ux, Uy, p1, p2 = 400
numbers; 183 of them above 1e-6 are compared): step 1 Ux = 1 and p(1) = 1 exactly
on both sides; worst
relative difference over the 183: **U 7.6e-9, p 3.5e-8** (asserted < 1e-7). Below 1e-6
the residual is dominated by the ~1e-14 state difference, not compared.

**Cause of the remaining ~1e-14, stated as a hypothesis, not proven:** the linear
solvers differ (the port's CG with its preconditioner and its Gauss-Seidel,
against upstream DIC-PCG and symGaussSeidel), both stopped at a 1e-14 residual.
Not isolated: that would need the same solver on both sides, which means porting
upstream's `symGaussSeidel` and `DICPreconditioner`, not done this round.
Bit-identity at step 1 is therefore NOT reached.

### Defects found and fixed (all by translating upstream)
- **D1. `Gauss linear` convection was not upstream's operator.** The port built
  implicit UPWIND plus a lagged explicit central correction
  (`solvers::schemes::div_vec_scheme`, `DivScheme::GaussLinear`); upstream
  `gaussConvectionScheme::fvmDiv` with the `linear` weights is fully implicit
  (`lower = -w*phi`, `upper = lower + phi`, `negSumDiag`). Inside a time step these
  differ, so `A`, `H`, `rAU` and everything downstream differed. New
  `fvm::div_vec_linear` (basic-lib) carries it. ABLATION: the old behaviour is the
  first-run row above (1.4e-3 at step 2). The unit test that asserted the old
  deferred-correction form (`linear_div_adds_the_exact_central_difference_correction`)
  now asserts the implicit one; changed because the old assertion is not upstream's operator.
- **D2. Pressure reference pinned with a 1e30 penalty.** Upstream
  `fvMatrix::setReference` does `source[c] += diag[c]*value; diag[c] += diag[c]`.
  `PimpleFoam` now uses `FvMatrix::set_reference_openfoam`; the old
  `set_reference` (1e30) is unchanged for the other solvers. Effect measured:
  p differs from the penalty version by < 5e-14 of max; its real cost was that the
  pressure initial residual was meaningless (about 1e-35, from the 1e30 in the norm).

### Added for this comparison
`LduMatrix::openfoam_initial_residual` (upstream `normFactor`), `PimpleFoam::
with_solve_log` / `SolveRecord` (opt-in, off by default), `FvMatrix::set_reference_openfoam`,
`fvm::div_vec_linear`.

### Not covered
Non-orthogonal meshes, `Gauss linearUpwind`/`limitedLinear` (the latter is not in the
port), nOuterCorrectors > 1, turbulence, and Foundation `pimpleFoam` (the
`incompressibleFluid` module) itself: icoFoam is the same algorithm by the port's
own statement, which this record does not independently verify. `phi` boundary
values are zero on both sides and are not compared.
