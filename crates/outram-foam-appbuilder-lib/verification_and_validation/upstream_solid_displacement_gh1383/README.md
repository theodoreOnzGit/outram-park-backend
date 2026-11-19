# solidDisplacementFoam vs ESI OpenFOAM v2506 (gh:#1383, epic #1336)

**Research, education and V&V only** (`RESPONSIBLE_USE.md`). AI-written draft, not human-reviewed.

## What this compares

Upstream: ESI OpenFOAM v2506 `applications/solvers/stressAnalysis/solidDisplacementFoam`
(`solidDisplacementFoam.C`, `createFields.H`, `readMechanicalProperties.H`,
`readThermalProperties.H`, `readSolidDisplacementFoamControls.H`, `calculateStress.H`), built
unmodified by the `stress` stage of `scripts/upstream/build-esi-v2506-and-genfoam.sh` into
`/home/user/upstream-build/gh1363/bin/solidDisplacementFoam` (only `Make/options` and `Make/files`
edited, as for the other solver copies).

Port under test: `solvers::solid_displacement_foam::SolidDisplacementFoamEsi`, a line-by-line
transcription of upstream's loop (compactNormalStress branch, `steadyState` d2dt2) on the
`outram-foam-basic-lib` operator layer where it is bit-identical (`interpolate`,
`grad_vec_corrected`, `DeltaCoeff`) and local ESI-form code elsewhere (matrix assembly with
upstream's boundary `internalCoeffs`/`boundaryCoeffs`, `fvMatrix::solveSegregated`, PCG with DIC,
`normFactor`).

The older `genfoam::thermo_mechanics::MechanicsMeshSolver` is a different algorithm (GeN-Foam's
`legacyThermoMechanics`, per-cell-change convergence, zero-gradient grad boundary) and is
compared too, as a second row, against the converged ESI answer.

## Case

`case/`: 0.6 x 0.4 x 0.3 m block, 6 x 4 x 3 cells with `simpleGrading (2 1 0.5)` (non-uniform
interpolation weights, orthogonal), E = 2e11, nu = 0.3, rho = 7854 (plane strain), C = 434, k = 60.5,
alpha = 1.1e-5. D: `left` fixedValue 0, `right` fixedValue (1e-4, 2e-5, -1e-5) m, other patches
zeroGradient. T (variant `thermal`): `left` 300 K, `right` 380 K fixed, others zeroGradient,
initial 300 K. `deltaT` 1000 s, 6 steps, `nCorrectors 4`, `D 1e-6`, PCG/DIC tolerance 1e-12 (D)
and 1e-14 (T), relTol 0, `Gauss linear` grad and div, `Gauss linear uncorrected` laplacians.
`writePrecision 17`, so the written ascii fields round-trip IEEE doubles exactly and the genuine
solver output is the reference (no instrumented driver). Variants: `thermal` (thermalStress yes), `mech` (thermalStress no), `mech3d` (as `mech`, top patch
fixedValue (0 3e-5 0), so the field is genuinely 3-D) and the 400-step `mechlong` / `mech3dlong`
(converged fixed points for row 2). Reference output of the real binary: `reference/<variant>/`
(fields D, T, sigma, sigmaEq at 1000 ... 6000 s, and the full log with 17-digit residuals).
Rerun: `scripts/run_solid_displacement.sh <thermal|mech>`.

Not covered (refused by name in the port): `compactNormalStress no` (needs `fvMatrix::flux()`),
transient `d2dt2` schemes, `tractionDisplacement`, `planeStress yes` is covered by the constants
only, non-uniform E/nu/rho fields, 2-D `empty` patches, GAMG.

## Predictions (committed BEFORE the port was run, 2026-11-14)

1. `SolidDisplacementFoamEsi` is a transcription with the same operation order, no FMA on either
   side (ESI built `-O3`, generic x86-64; Rust does not contract), same accumulation orders.
   **Prediction: D, T, sigma, sigmaEq bit-identical at all six steps, both variants (zero signs
   aside); PCG iteration counts identical on every solve; initial and final residuals in the
   log identical to all 17 printed digits.** Round-off source I would expect if it misses: the
   `normFactor` or DIC sums, `1/3` in `dev`, `sqrt`. A miss is reported with the first differing
   quantity, not tuned.
2. `MechanicsMeshSolver` is **not** expected to match: its `fvc::grad_vec` leaves a zero-gradient
   boundary, so the explicit `divSigmaExp` carries no boundary gradient information, and it uses an
   orthogonal-only `laplacian_vec` with a different convergence criterion. **Prediction: relative
   difference in D from the converged ESI field of order 1e-3 to 1e-1 of the displacement
   amplitude, largest in the cells next to the fixedValue patches, non-zero; the difference is
   in the discretisation, not round-off.**

## Results (measured 2026-11-14, release, x86_64, glibc; ESI v2506 `_615aae61d7-20250627`)

Run: `cargo test --release -p outram-foam-appbuilder-lib --test upstream_solid_displacement_gh1383 -- --nocapture`
(6 tests, all pass). Reference reruns with `scripts/run_solid_displacement.sh`.

### Row 1: `SolidDisplacementFoamEsi` vs the real binary (prediction 1: CONFIRMED)

Per cell, per step, bit patterns compared (`+0.0 == -0.0` counted equal); 6 steps x 4 correctors.

| variant | D (3 x 72 x 6) | T (72 x 6) | sigma (6 x 72 x 6) | sigmaEq (72 x 6) | linear solves exact (init, final residual, iterations) |
|---|---|---|---|---|---|
| `mech` (thermalStress no) | 1296/1296 | n/a | 2592/2592 | 432/432 | 72/72 |
| `mech3d` (top patch held, genuinely 3-D) | 1296/1296 | n/a | 2592/2592 | 432/432 | 72/72 |
| `thermal` (T solved, `grad(threeKalpha*T)`) | 1296/1296 | 432/432 | 2592/2592 | 432/432 | 96/96 |

Largest difference of any quantity: 0 (worst_abs 0.0 in every tally). Every residual is compared
to the 17 digits the log prints, and every PCG iteration count (0 to 15 per solve; 0 where the guess already satisfies the tolerance) is equal; the
convergence test on the maximum component initial residual is exercised (all four correctors run
in these 6 steps, because the residual stays above `D 1e-6`; the early-exit branch is not hit).
A control test shows the tally can fail (the mechanical port against the thermal reference).
Methodology: same `polyMesh` (written by ESI `blockMesh`), same constants, `Gauss linear` grad and
div, `Gauss linear uncorrected` laplacians, PCG/DIC with tolerance 1e-12 (D) and 1e-14 (T), relTol 0.

No round-off allowance was needed: no residual difference exists to flag for the maintainer.

### Row 2: `MechanicsMeshSolver` (GeN-Foam route) vs the converged ESI field (prediction 2: PARTLY WRONG)

Quasi-static solve to its fixed point (48 to 55 correctors, tolerance 1e-15 m) against the ESI
fixed point (400 steps x 4 correctors, last initial residual 1e-12).

| case | worst abs difference in D | / amplitude (8.85e-5 m) | worst relative (per component) |
|---|---|---|---|
| `mechlong` (field depends on x only) | 1.36e-15 m | **1.5e-11** | 2.4e-10 |
| `mech3dlong` (top patch held) | 8.46e-6 m | **9.6e-2** | 0.20 |

Prediction 2 said 1e-3 to 1e-1 of the amplitude on the displacement-driven case. That held for
the three-dimensional case (9.6e-2) and was **wrong for the one-dimensional case** (1.5e-11): with
a uniform end displacement and zero-gradient sides the converged field varies only along x, the
explicit transverse stress terms vanish, and the boundary-gradient difference has nothing to act
on (the module doc of `mesh_solve.rs` already said so for a 1-D column; I did not apply it to my
own case when I wrote the prediction).

**Cause, localised (not tuned):** `fvc::grad_vec` leaves the boundary gradient at the owner-cell
value, whereas upstream's `gaussGrad::correctBoundaryConditions` replaces the wall-normal part by
the patch's own `snGrad`. The new builder `SolidDisplacementFoamEsi::without_boundary_gradient_correction()`
removes only that correction. Run to its fixed point it differs from `MechanicsMeshSolver` by
**1.6e-15 m** (relative 1.5e-9 on the smallest components) and from the ESI binary by
**8.46e-6 m**, the same number as the row above. So the correction accounts for all of the
9.6 % and none of the remaining algebra (constants, load construction, laplacian, convergence
criterion) differs measurably. Consequence: `MechanicsMeshSolver` should not be used where
transverse boundary stresses matter until it adopts `grad_vec_corrected`; that is a change to a
possibly reviewed solver and is **deferred to the maintainer** (filed as a child issue).

### What is not covered

`compactNormalStress no`, transient `d2dt2` (Euler/backward), `tractionDisplacement` and so
`plateHole` itself (needs `symmetryPlane` and `tractionDisplacement` patches in the port), field-valued
`E`/`nu`/`rho`, `empty` patches (2-D), GAMG, `solidEquilibriumDisplacementFoam`, non-orthogonal
meshes with `corrected` laplacians. The comparison is on one small orthogonal graded block; the
agreement says the operation order matches upstream's on this mesh, not that every branch is
exercised.
