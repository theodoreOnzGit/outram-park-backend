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
solver output is the reference (no instrumented driver). Variants: `thermal` (thermalStress yes)
and `mech` (thermalStress no). Reference output of the real binary: `reference/<variant>/`
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

## Results

(filled in after the run)
