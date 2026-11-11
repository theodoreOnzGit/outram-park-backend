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

## Results
(to be filled after the port run)
