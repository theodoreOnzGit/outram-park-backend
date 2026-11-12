# `rho_pimple_foam` vs upstream rhoPimpleFoam, h-form tube (gh:#1363, row 3a)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

## Which upstream, and what exactly was run
ESI **OpenFOAM-v2506** `applications/solvers/compressible/rhoPimpleFoam`
(`rhoPimpleFoam.C`, `createFields.H`, `UEqn.H`, `EEqn.H`, `pEqn.H`; the
non-transonic, non-consistent branch). The Foundation line moved the solver
into the `fluid` module (`foamRun -solver fluid`); the existing
`RhoPimpleFoam::step` cites `applications/solvers/compressible/rhoPimpleFoam/`
but is a sketch of neither line (no `K`, no `dpdt`, no `ddtCorr`, `rho`
and `alphaEff` as free fields). It stays callable unchanged.

`driver/rhoPimpleDump.C` is `rhoPimpleFoam.C` with those `.H` files transcribed
line for line on the **unmodified** ESI libraries (real `fluidThermo::New` giving
`hePsiThermo`, real `compressible::turbulenceModel::New`, laminar). Substitutions,
all of them: `fvOptions`, MRF, mesh motion (`dynamicFvMesh`, `CorrectPhi`,
`rhoUf`, LTS), `pressureControl` (no fixed-value pressure patch, so it never
limits) and `makeRelative` are omitted because the case has none and the
libraries are not part of the minimal closure; `runTime.write()` is replaced by
hex dumps; continuity errors are also printed as bit patterns.
**It is not the unmodified `rhoPimpleFoam` binary.**

**Case: not the shipped tutorials.** `laminar/sineWaveDamping` needs Sutherland
transport (viscous), `backward` ddt, `linearUpwind`, `nOuterCorrectors 7`,
`fvOptions acousticDampingSource` and a `fieldAverage` function object, none of
which is in the minimal closure. The smallest equivalent used instead is the
sonicFoam laminar shockTube mesh and states (100 cells, committed polyMesh,
T = 348.432 K / p = 1e5 Pa left, 278.746 K / 1e4 Pa right, `hConst` Cp = 1005,
molWeight 28.9, perfectGas, **`mu = 0`**) with **`sensibleEnthalpy`**, so the
h-form `-dpdt` branch and the compressibility pressure equation
`ddt(rho) + psi*correction(ddt(p)) + div(phiHbyA) - laplacian(rhorAUf, p)` run;
Euler ddt, `div(phi,U)` upwind, `div(phi,h)`, `div(phi,K)` limitedLinear 1,
`Gauss linear orthogonal`, `smoothSolver symGaussSeidel` at tolerance 1e-15,
`nOuterCorrectors 2`, `nCorrectors 1`, fixed dt = 1e-5, 700 steps. The
viscous, backward-ddt and `linearUpwind` paths of rhoPimpleFoam are therefore
**not** compared by this row.

## Method
`tests/upstream_rho_pimple_gh1363.rs` reads the committed polyMesh and
`shock/reference/rhopimple_v2506_shockTube.txt.gz` (bit patterns of rho, Ux, T,
p, h, psi, internal-face phi at steps 1-5, 10, 100, 700; continuity errors of
every pressure corrector; the `smoothSolver` log of steps 1-10), runs the port
and counts bit-identical values.

## Predictions (written 2026-11-12, BEFORE the port was run)
Read: the five upstream files above, `EulerDdtScheme.C` (`fvmDdt`, `fvcDdt(vf)`,
`fvcDdtPhiCorr`), `fvMatrix.C` (`operator&`, `correction`), `lduMatrix`
operators, `perfectGasI.H`, `hConstThermoI.H`, `thermoI.H` (`T`), and the
port's `solvers/sonic_foam/esi.rs` (row 3b, which matched sonicFoam bit for bit
for 700 steps and from which this port reuses every shared operator).

1. The existing `RhoPimpleFoam::step` is not comparable (see above); not run.
2. A port that reuses the row-3b operators and translates the differences below
   is **bit-identical** (all fields, solver logs, continuity errors, 700 steps);
   no `pow`/trig is used, so no 1-ulp PETIR allowance is expected.
3. Differences from sonicFoam, each a place a divergence would first show:
   - **`dpdt`**: `createDpdt.H` builds `dpdt = fvc::ddt(p)`, which creates
     `p.oldTime()` at start-up, so `dpdt = 0` in the first `EEqn` of step 1; it is
     re-evaluated at the end of every `pEqn` as `rDeltaT*(p - p0)` with `p0` the
     value at the start of the step. `EEqn` subtracts it: `source += V*dpdt`.
   - **`rho = thermo.rho()` after the continuity solve** (end of `pEqn.H`): the
     cell `rho` of the second outer iteration is `p psi`, not the solved `rho`
     (sonicFoam keeps the solved `rho`).
   - **`rhoEqn` only at the first outer iteration** and after every `pEqn`.
   - **No `psi.oldTime()`**: `psi*correction(fvm::ddt(p))` uses `ddt(p)`.
   - `psi*correction(fvm::ddt(p))`: `correction(A) = A - (A & p)`, where
     `(A & p) = (-diag*p + source)/(-V)` and `A - field` adds `V*field` to the
     source; then the matrix is multiplied by `psi` cell by cell.
   - The pressure matrix is **symmetric**: `diag - (-laplacian)` with `lower == upper`
     (upstream `operator-=` of a diagonal by a symmetric matrix stores `upper = -B.upper`);
     face flux `phi = phiHbyA + pEqn.flux()`.
   - `phiHbyA = interpolate(rho)*flux(HbyA) + rhorAUf*ddtCorr(rho,U,phi)`; the
     boundary `ddtCorr` coefficient is 0 (found in row 3b: the `rhoU0` field has
     `calculated` patches, which `fixesValue()`).
   - `K.oldTime()` is created late as in sonicFoam (row 3b defect 1).
   - Enthalpy: `Hs = Cp (T - Tref)`, Newton `T(h, p, T0)` with `F = Hs`, `dFdT = Cp`
     (not `Cv`); `h_b = h_c` on the `gradientEnergy` patch, as for `e`.
4. If the port misses, the first divergence will be located with the stage dumps
   (`dumpStages yes`, matrices and fields) as in row 3b.

## Results
(Recorded below after the run.)
