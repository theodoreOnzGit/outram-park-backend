# `sonic_foam` vs upstream sonicFoam, laminar shockTube (gh:#1363, row 3b)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

## Which upstream, and what exactly was run
ESI **OpenFOAM-v2506** `applications/solvers/compressible/sonicFoam`
(`sonicFoam.C`, `createFields.H`, `UEqn.H`, `EEqn.H`, `pEqn.H`). The Foundation
line has no `sonicFoam` (it is the `compressibleVoF`/`fluid` modules), so there
is no Foundation form to keep: the existing `SonicFoam::step` is a sketch of
neither line (no energy equation, explicit `psi`-convection), and stays callable
unchanged as the dev form.

`driver/sonicDump.C` is `sonicFoam.C` plus its four `.H` files transcribed line
for line, linked against the **real, unmodified** ESI libraries, including the
real `psiThermo::New` (`hePsiThermo`) and the real
`compressible::turbulenceModel::New` (laminar). Substitutions, all of them:
1. `fvOptions` and `MRF` are omitted (libfvOptions and its closure are not
   built, see "Build cost"); the shockTube has neither. `rhoEqn.H` is therefore
   inlined without `fvOptions(rho)`, which is an empty source there.
2. `runTime.write()` is replaced by hex dumps; continuity errors are also
   printed as bit patterns.
3. `compressibleTurbulenceModels` was linked from a scratch copy with three
   unbuilt `-l` entries removed (see "Build cost").
**It is not the unmodified `sonicFoam` binary.**

Case: v2506 tutorial `compressible/sonicFoam/laminar/shockTube` with the mesh
reduced from 1000 to **100 cells** (committed polyMesh, small reference) and a
fixed `deltaT = 1e-5`, **700 steps** (t = 0.007 s, the tutorial end time).
`setFields` is reproduced by writing the same two states (T = 348.432 K,
p = 1e5 for x < 0; T = 278.746 K, p = 1e4 for x >= 0). Schemes and solvers are
the tutorial's: Euler; `div(phi,U)` upwind; `div(phid,p)`, `div(phi,e)`,
`div(phi,K)`, `div(phiv,p)` limitedLinear 1; `Gauss linear orthogonal`;
`smoothSolver symGaussSeidel` for `(p|U|e).*` and `PCG DIC` for `rho.*` both at
`tolerance 1e-15, relTol 0` (the `rho` matrix is diagonal, so upstream's
`lduMatrix::solver::New` picks `diagonalSolver`: the `PCG` entry is never used);
`nOuterCorrectors 2`, `nCorrectors 1`. Gas: `hePsiThermo`, `const` transport
with mu = 0, `hConst` Cp = 1005, perfectGas, molWeight 28.9,
`sensibleInternalEnergy`.

## Scope of the port (said up front)
`SonicFoamEsi` (new, `src/solvers/sonic_foam/esi.rs`) is a translation for a
mesh whose non-empty patches are all `zeroGradient` for U, p, T and whose
boundary mass flux stays exactly zero. It **refuses** (a named error) a viscous
gas (mu > 0, so no `divDevRhoReff`/`alphaEff` branches), any other patch type,
and a non-zero boundary flux at any step. It does not implement
`fvOptions`, MRF, mesh motion or turbulence models.

## Method
`tests/upstream_sonic_gh1363.rs` reads the committed polyMesh and the gzipped
reference (`shock/reference/sonic_v2506_shockTube.txt.gz`, IEEE-754 bit
patterns of rho, Ux, T, p, e, psi and the internal-face `phi` at steps 1-5, 10,
100, 700, the geometry, and the continuity errors), runs the port, and counts
bit-identical values and the worst difference per field.

## Predictions (written 2026-11-12, BEFORE the port was run)
Read: the five upstream files, `EulerDdtScheme.C`, `ddtScheme.C`,
`gaussConvectionScheme.C`, `LimitedScheme.C`, `limitedLinear.H`, `NVDTVD.H`,
`fvMatrix.C` (`H`, `A`, `D`, `solveSegregated`), `smoothSolver.C`,
`symGaussSeidelSmoother.C`, `lduMatrix` `Amul`/`residual`/`sumA`/`H`,
`solver::normFactor`, `SolverPerformance::checkConvergence`.

1. **Existing `SonicFoam::step`** cannot be compared: it has no energy equation,
   no thermo, and convects `psi p` explicitly. Not run against this reference.
2. **`SonicFoamEsi`, if every operation is translated in upstream's order,
   bit-identical** (rho, Ux, T, p, e, psi, phi at every dumped step, worst
   difference 0.0). The case uses no `pow`/trig (only `+ - * /`), so the
   1-ulp PETIR allowance should not be needed.
3. The translation points where a plausible port goes wrong (each is a place a
   divergence would first show; ordered by where in step 1 it appears):
   - `rDeltaT = 1.0/deltaT` then `rDeltaT*rho*V`, not `rho*V/dt`: differs at 1 ulp in
     the `ddt` diagonal and source of every cell with some probability.
   - **`psi.oldTime()` in step 1.** Upstream creates it on first use, inside
     `fvm::ddt(psi, p)` of the first outer iteration, as a copy of `psi` *after*
     that iteration's `thermo.correct()`. From step 2 it is the end-of-previous-step
     value. A port that stores `psi` at step start gets step 1 wrong at the
     jump cells (psi differs by O(1e-9) relative there since e is solved
     in the first outer iteration), and step 2 right.
   - `limitedLinear` limiter from `fvc::grad` of the **solved field** (e, K, p),
     with `d = C[nei] - C[own]` and the `1000*|gradf|` branch; boundary limiter 1.
   - `symGaussSeidel` forward-then-backward sweep, `normFactor`
     (`sumA * average(psi)`), residual sum in face order, convergence test
     `finalResidual < 1e-15`; the iteration counts logged by upstream (1-2 per
     solve) must be reproduced.
   - the `ddtCorr` coefficient `1 - min(|phiCorr|/(|phi| + SMALL), 1)`, with
     `rhoU0 = rho.oldTime()*U.oldTime()`, and `rDeltaT*coeff*phiCorr` operand order.
   - `A()` = `D()/V` and `H()` with `lduMatrix::H` face order.
4. **If the translation misses**, the first divergence will be at step 1 in the
   fields of the first equation that differs, located with the stage dumps
   (`dumpStages yes`: rho after rhoEqn, Ux after UEqn, e/T after EEqn, rAU, HbyA,
   phid and rhorAUf, p, phi, Ux after pEqn).

## Results (2026-11-12, release, 100 cells, 700 steps)
`SonicFoamEsi` (`src/solvers/sonic_foam/esi.rs`) against the driver, gate test
`tests/upstream_sonic_gh1363.rs::shock_tube_matches_upstream_sonicfoam`:

| quantity | result |
|---|---|
| geometry V, Sfx, magSf, w, deltaCoeffs | bit-identical (100/100, 99/99) |
| rho, Ux, T, p, e, psi at steps 1, 2, 3, 4, 5, 10, 100, 700 | **800 of 800 bit-identical each, worst difference 0.0** |
| face flux phi (99 internal faces) at the same steps | **792 of 792 bit-identical** |
| `smoothSolver` log (initial residual, final residual, iterations) of every solve in steps 1-10 | **identical to the last bit** |
| continuity errors (sum local, global) of all 1400 pressure correctors, steps 1-700 | **0 mismatches** |

The prediction (bit-identity, no 1-ulp allowance, no pow/trig in this case)
held. The first run missed (see below), so the prediction was capable of failing:
the first port run had `e` wrong by 6e-3 of max at step 1.

### Defects, each located by the first divergent stage and fixed by translating upstream
1. **`K.oldTime()` created late.** `createK.H` creates the old-time K only
   `if (U.nOldTimes())`, false at start-up, so `K.oldTime()` is first requested by
   `fvc::ddt(rho, K)` in the first `EEqn`, as a copy of the K just set by the
   momentum predictor: `ddt(rho,K) = 0` in the first iteration of step 1. First
   run: step 1 `e` off by 6.5e-3 of max, `e` solver residual 3e-5 vs 6e-17.
2. **`psi.oldTime()` created on first use** (prediction 3b, confirmed): step 1's
   `psi0` is psi after the first `thermo.correct()`.
3. **y and z round-off of `Sf`, cell centres and U are carried.** The limiter's
   `d & grad` reacts to them: 9 of 99 `e` matrix coefficients (weights 0.5 vs 1)
   differed in step 1 until they were carried; the fields were already identical,
   only the final solver residual (round-off sums) differed in the 5th digit.
4. **Boundary mass flux of the closed ends** is ~1e-12 kg/s before any wave
   arrives (the implicit pressure equation) and changes the end-cell `e` source by
   1 ulp from step 40 on. Translated: boundary `phi_b`, `rho_b` (as last assigned
   by `rho = thermo.rho()`), `internalCoeffs` added to the diagonal
   (`addBoundaryDiag`), `cmptAv(internalCoeffs)` in `A()` and `H()`, and `phid_b`.
5. **`fvcDdtPhiCoeff` zeroes the coefficient on boundary patches whose field
   `fixesValue()`.** Here that field is `rhoU0 = rho.oldTime()*U.oldTime()`, whose
   patches are `calculated`, and `calculatedFvPatchField::fixesValue()` is true: the
   boundary `ddtCorr` is exactly 0. The port had a non-zero boundary coefficient
   (0.015) and `phid_b` was 10 % off from step 94, first visible as a 1-ulp `e`
   difference at step 95 (first divergent step found by comparing every step 1-120;
   stage probe at step 95: `phi0_b` 6.1e-14 vs 6.7e-14).

### Real binary
The real `sonicFoam` binary (link line without `-lthermoTools`, which it does not
use; `esi_thermo_turbulence_closure_build.md`) was run on the same case: its 6300
solver log lines and 1400 continuity-error lines (17 digits) are identical to the
driver's, which validates the driver's omission of `fvOptions`/MRF on this case.

### Scope and honest limits
* The reference is the driver (see above), checked against the real binary by
  log identity, not field by field.
* 100 cells instead of the tutorial's 1000; fixed dt; the waves do not reach the
  closed ends in 700 steps (end-cell speed stays ~1e-6 m/s). Bit-identity of
  the end-cell terms comes from translating the zeroGradient boundary
  coefficients; a case where the waves reach the ends is not tested.
* Inviscid (mu = 0) only, `Euler`, `smoothSolver symGaussSeidel`, `limitedLinear 1`
  and `upwind`; other schemes are refused or absent, not silently substituted.
* No transcendental function is used (only `+ - * / sqrt`), so PETIR's pow/trig
  1-ulp allowance was not needed.
* Diagnostics kept: `stage_probe_against_upstream_driver_log` and
  `first_divergent_step_against_upstream_driver_log` read a local driver log from
  `scripts/run_sonic.sh` (`NOREF=1`, `DUMPSTAGES=yes`, `DUMPSTAGEFROM/STEPS`) and do
  nothing when it is absent.
