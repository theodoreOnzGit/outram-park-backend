# `reacting_two_phase_euler_foam` vs ESI v2506 reactingTwoPhaseEulerFoam / multiphaseEulerFoam (gh:#1363, row 5)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

## ~~Status: NOT RUN. Blocked by disk and by a structural mismatch (2026-11-12)~~ **CORRECTED 2026-11-13 (gh:#1363, round 2): RUN, see "Round 2" below**
Round 1 was blocked by disk and by an estimate of 280 library sources. The legacy ESI `twoPhaseEulerFoam` needs 92 sources, built in 6.5 minutes with 7 GB free; the reduced bubbleColumn was run and compared. The round-1 text is kept as written.
No upstream run and no comparison were made for this row. What follows is what
was checked, so that the next attempt does not repeat it. Everything marked
"by reading" is a reading of source, not a measurement.

### 1. The ESI counterpart exists
ESI v2506 has `applications/solvers/multiphase/reactingTwoPhaseEulerFoam` (the
direct counterpart: `phaseProperties` with `basicTwoPhaseSystem`, as in the
shipped `tutorials/multiphase/reactingTwoPhaseEulerFoam/laminar/bubbleColumn`)
and `multiphaseEulerFoam` (`tutorials/multiphase/multiphaseEulerFoam/bubbleColumn`).
The Foundation `multiphaseEuler` module, which `ReactingTwoPhaseEulerFoam`'s
header cites, is not what ESI ships.

### 2. What building it needs, measured from the source tree
`reactingTwoPhaseEulerFoam`'s `Make/options` links `-lreactingMultiphaseSystem
-lreactingTwoPhaseSystem -ltwoPhaseReactingTurbulenceModels -lthermoTools`, and
those libraries include `combustionModels`, `saturationModel` and
`chemistryModel`. Source file counts (`find -name '*.C'`):

| library | .C files | built? |
|---|---:|---|
| `phaseSystemModels/reactingEuler/multiphaseSystem` (reactingMultiphaseSystem) | 173 | no |
| `.../twoPhaseSystem` | 11 | no |
| `.../twoPhaseCompressibleTurbulenceModels` | 25 | no |
| `.../saturationModels` | 7 | no |
| `combustionModels` | 32 | no |
| `thermophysicalModels/chemistryModel` | 32 | no |
| `thermoTools` | n/a | no (needs `incompressibleMultiphaseSystems`) |

About 280 files of the phase-system template family, against the 340 files and
29 min (2 cores, shared) measured for the 19-library thermo/turbulence closure
(`esi_thermo_turbulence_closure_build.md`). That estimate is by analogy, not a
measurement. The real `reactingTwoPhaseEulerFoam` binary (no dump driver
needed: the solver loop lives in the application and the phase system in the
libraries) could probably be linked with `thermoTools` pruned, as was done for
`rhoPimpleFoam`; not tried.

### 3. Disk
`df -h /` read **1.4 GB free (97 %)** at the time of the decision, against the
3 GB floor, with the shared cargo target at 11 GB. No build was started.

### 4. The port is not a translation of upstream's phase system (by reading)
`ReactingTwoPhaseEulerFoam` composes `outram_foam_multiphase::TwoFluidPimple`,
whose own header says: drag is the only interfacial force in the coupling; lift,
virtual mass, wall lubrication and turbulent dispersion are scaffolds; the
partial-elimination pressure-velocity coupling (`momentumTransferSystem::invADVs`)
is not ported; the system is incompressible and isothermal in its hydrodynamic
core. The shipped `bubbleColumn` `phaseProperties` has linear blending between
`air in water` and `water in air` drag (`SchillerNaumann` both ways, plus a
`segregated` pair model), `constantCoefficient` virtual mass with `Cvm 0.5`, an
`isothermal` bubble diameter model for air, `RanzMarshall` heat transfer, constant
surface tension and compressible air. Of these the port has `SchillerNaumann` and
`RanzMarshall` (diameter models: not checked); it has none of blending, segregated drag,
virtual mass, the isothermal diameter, or compressible phases.

**Consequence for the prediction.** Field-level agreement on the tutorial case
is not expected at any step after the first pressure solve, and the first
divergent closure would be the virtual-mass term in the momentum predictor, then
partial elimination in the pressure equation. A fair comparison therefore needs
either a port of those two pieces (a large change to a crate this task does not
own) or a deliberately reduced upstream case (`virtualMass` and `blending` off,
incompressible phases, `diameterModel constant`) so the closures actually match.
The second is the cheaper first step once the libraries are built.

## What was not done
Everything: no library built, no case run, no port run, no number measured.
No port code, test or tolerance changed for this row.

---

# Round 2 (2026-11-13): reduced bubbleColumn on ESI v2506 `twoPhaseEulerFoam`

Predictions below were written and committed BEFORE the ESI case or the port
was run. Research, education and V&V only (`RESPONSIBLE_USE.md`).

## Which upstream solver, and why not the one named in round 1
Round 1 planned the `reactingTwoPhaseEulerFoam` family (about 280 library
sources). ESI v2506 also ships the legacy **`twoPhaseEulerFoam`**
(`applications/solvers/multiphase/twoPhaseEulerFoam`, the `pU` branch,
`faceMomentum no`), which needs only `compressibleTwoPhaseSystem` (67 sources)
and `phaseCompressibleTurbulenceModels` (25), on top of libraries already
built. It is the same algorithm class (partial elimination, p_rgh, MULES alpha).
New stage `twophase` in `scripts/upstream/build-esi-v2506-and-genfoam.sh`
builds only those two libraries and the solver (scratch copy, `thermoTools`
pruned from the link line, as for `rhoPimpleFoam`), `nice -n 10`, 2 jobs.
The Foundation `multiphaseEuler` module, which the port's headers cite, was
not built or run.

## Reduced case (`upstream_openfoam_gh1363/bubble/`, written by `scripts/make_bubble_case.sh`)
Box 0.15 x 0.30 x 0.10 m, 6 x 12 x 1 hexes, four wall sides, front/back
empty (a 2-D closed tank). Each simplification and why:
1. **Closed tank, no inlet/outlet.** The port cannot impose a fixed-pressure
   outlet (its module doc), so the tutorial's inlet/outlet is not reproducible.
2. **No free surface:** water everywhere with a bubble cloud,
   `alpha.air = 0.10 + 0.15 exp(-r^2 / (2 * 0.03^2))` about (0.075, 0.075), so
   0.10 <= alpha.air <= 0.25 < `maxFullyDispersedAlpha` 0.3. Both residual
   alphas (1e-6) are far from active and the tutorial's linear blending would
   reduce to the dispersed model; the case uses `blending none` (continuous
   phase water) so only "air in water" drag is evaluated.
3. **Drag only:** `virtualMass ( )`, `lift ( )`, `wallLubrication ( )`,
   `turbulentDispersion ( )`: the port's force set.
4. **Constant density (`rhoConst`, air 1.2, water 1000 kg/m3)**: the port is
   incompressible. Tutorial water is `perfectFluid`, air `perfectGas`.
5. ~~**Isothermal in effect:** both phases 300 K, `RanzMarshall` kept (needed by
   the solver) but its driving temperature difference is zero. ESI still
   solves its energy equations; they cannot feed back with `rhoConst`.~~
   **CORRECTED 2026-11-13:** ESI's energy equations diverged (negative T at
   step 8, also with air `Cv = 1e7`), so the dump copy of the solver does not
   solve them (`EEqns.H` skipped, in `driver/twoPhaseEulerDump/*.diff`). They
   cannot feed back with `rhoConst`. `RanzMarshall` is still read.
6. **Constant diameters** (air 3 mm, water 0.1 mm) instead of the tutorial's
   `isothermal` air diameter (the port has no pressure-dependent diameter).
7. **Laminar, real viscosities** (air 1.84e-5, water 1e-3 Pa s). The port has
   no `dev2(T(grad U))` stress term; ESI has it (scheme `Gauss linear`). This
   is NOT removed from the case (nu = 0 would break the drag Reynolds number),
   so it is a known source of difference, bounded by prediction P6.
8. **All-upwind schemes, `Euler`, `nAlphaSubCycles 1`, `nAlphaCorr 1`,
   1 outer and 1 pressure corrector**, linear solvers at 1e-15, `relTol 0`.
   ~~linear solvers at 1e-15~~ **CORRECTED 2026-11-13:** `p_rgh` is `PCG` with a
   `diagonal` preconditioner at 1e-12 and the pressure level is 0 with
   `pMin -1e9` (instead of 1e5 and 1e4): the closed tank with `rhoConst` has a
   singular pressure matrix, on which `DICPCG` at 1e-15 stalled; see "The ESI
   case needed three corrections".
9. **no-slip walls on both phases** (tutorial: fixedValue 0 on `walls`).
10. dt = 1e-3 s (tutorial 5e-3), 10 steps; also dt = 1e-4 for the scaling check.

## Hand estimates the predictions rest on (derived before running)
- Newton-regime terminal slip for d = 3 mm: `Ur = sqrt(4 d g (rho_w - rho_a) /
  (3 Cd rho_w))`, Cd = 0.44: 0.30 m/s (Re about 900).
- Air-phase drag relaxation time `tau = rho_a / (K/alpha_a)`, with `K/alpha_a =
  0.75 Cd rho_w Ur / d` = 3.3e4 1/s: tau = 3.6e-5 s, so dt/tau = 28 at
  dt = 1e-3. The air momentum is quasi-steady within one step: the slip is
  already near terminal at step 1 in any consistent scheme.
- Closed tank, zero net volume flux: `alpha_a U_a + alpha_w U_w` has zero
  divergence, so in the blob `U_w` is about `-alpha_a Ur` (-0.03 to -0.08 m/s)
  and `U_a` about `(1 - alpha_a) Ur` (0.22 to 0.27 m/s).
- Courant number `Ur dt / dy` = 0.012 (dt = 1e-3); alpha change per step
  about `Co * (alpha difference between cells 0.03)` = 4e-4.

## Predictions: existing port `TwoFluidPimple::solve_timestep` vs ESI
(port: 1 outer, 1 corrector, same mesh, same alpha, same properties.)
- **P1** Step 1, max |U.air_y| over cells within 25 % of ESI; max |U.water_y|
  within 25 % (the semi-implicit drag with explicit other-phase velocity and
  the predictor-with-grad(p) form differ from partial elimination; the 28x
  stiffness makes the split visible).
- **P2** Step 1, alpha.air: worst absolute difference between 1e-6 and 3e-4
  (explicit vs implicit upwind, about Co * 4e-4 = 5e-6, plus ESI's
  relative-flux form).
- **P3** Step 1, p minus its volume mean: worst difference within 10 % of the
  range of p in ESI (hydrostatic part dominates and is the same).
- **P4** Bit-identical cells in any field: 0.
- **P5** After 10 steps, the U.air difference is not smaller than at step 1
  (relative, same metric); alpha difference grows to under 2e-3.
- **P6** The `dev2(T(grad U))` term (neglected in the port) is below 1e-3 of the
  momentum balance: nu_w |grad div U| / g with nu_w = 1e-6, so it cannot be
  the first divergence.
- **P7** dt scaling: reducing dt by 10x reduces the alpha difference by at
  least 3x (an O(Co) effect) but does NOT reduce the U.air difference by 3x
  (structural, not a time-step effect).

## Predictions: ESI-form translation to be added to the port
(`solve_timestep_esi`: `pU/UEqns.H`, `pU/pEqn.H`, `twoPhaseSystem::solve()`
with explicit alpha, Cvm = 0 here.)
- **P8** Step 1: every field (alpha.air, U.air, U.water, p_rgh) within 1e-10 of
  the field peak (linear solvers differ, both at 1e-15), p compared after
  removing the reference constant. 10 steps: within 1e-9.
- **P9** Bit-identical cells: under 25 %.
- **P10** The first divergent operation, if P8 misses, will be the pressure
  reference handling or the `ddtPhiCorr` filter, not a formula in the momentum
  matrix.

## Round 2 results (2026-11-13; AI-run, not human-reviewed)

Test: `tests/upstream_bubble_gh1363.rs` (7 tests). Harness:
`upstream_openfoam_gh1363/{scripts/make_bubble_case.sh, scripts/run_bubble.sh,
driver/twoPhaseEulerDump/, bubble/}`. References: `bubble/reference/*.txt.gz`
(17-digit text dumps of the real ESI solver). Build cost of the `twophase`
stage: `compressibleTwoPhaseSystem` 251 s, `phaseCompressibleTurbulenceModels`
109 s, solver 34 s (2 jobs, `nice`), about 7 MB of libraries, free disk stayed
at 7 GB (round 1 estimated 280 sources and a 3 GB disk risk; the legacy solver
needs 92).

### What was compared
Per cell and per step, the port against the dump of ESI v2506
`twoPhaseEulerFoam`: `alpha.air`, `U.air`, `U.water` (all three components),
`p` (as `p_rgh + rho*gh`, both sides minus their cell-0 value, because a closed
tank fixes p only up to a constant), and at steps 1, 2 (dt = 1e-3) and 3
(dt = 1e-4) every intermediate of the pressure-velocity coupling: `Kd`, `rAU`,
`HbyA`, `phiHbyA1/2`, `phiHbyA`, `rAUf`, `phig`, `alpha*rAU` face values,
`rho`, `p_rgh`, the phase fluxes.

### Table 1: the existing dev-form `TwoFluidPimple::solve_timestep` vs ESI
Worst |port - ESI| / field peak per step (alpha: absolute). 1 outer, 1 corrector,
linear solvers at 1e-14.

| dt | step | alpha (abs) | U.air | U.water | p (cell 0 removed) |
|---|---|---|---|---|---|
| 1e-3 | 1 | 2.6e-4 | 46 % | 47 % | 0.4 % |
| 1e-3 | 2 | 6.7e-4 | 26 % | 52 % | 3.8 % |
| 1e-3 | 5 | 2.3e-3 | 22 % | 65 % | 3.5 % |
| 1e-3 | 10 | 5.5e-3 | 23 % | 75 % | 12 % |
| 1e-4 | 1 | 2.6e-6 | 49 % | 49 % | 0.4 % |
| 1e-4 | 10 | 9.1e-5 | 36 % | 72 % | 14 % |

Bit-identical: none (the few "identical" entries of the raw table are zeros).
The dev form is a different algorithm from ESI's, not a rounding variant: its
relative error does not shrink with dt (46 % at 1e-3, 49 % at 1e-4 at step 1).

### Defects and structural differences found, and what was done
The port is not a translation of `twoPhaseEulerFoam`; the first divergence is
at the first pressure-velocity coupling. Each item below was found by a
differing intermediate, and is fixed in the new ESI-form step
`outram_foam_multiphase::two_fluid_pimple_esi::TwoFluidPimpleEsi::solve_timestep_esi`
(new code; the dev form is unchanged and callable; no reviewed function was
touched, `review.md` of the crate has no `kind = "review"` block).

| # | difference (dev form vs ESI) | evidence | ESI-form option |
|---|---|---|---|
| D1 | alpha is advanced LAST on the new velocity (ESI: FIRST, on the previous step's fluxes, so alpha at step 1 is the initial alpha exactly) | step 1 alpha off by 2.6e-4 | translated: `twoPhaseSystem::solve()` |
| D2 | implicit upwind alpha, clamp only; ESI is explicit with the relative-flux compression term `phir` and the MULES limiter | with the limiter off, alpha at step 2 is off by 3.2e-4 in the six bottom-row cells (the limiter stops them losing air through the wall-side relative flux); with it on, 8.7e-15 | translated: `MULES::limiter` (3 iterations, no extrema coefficients) |
| D3 | a momentum solve with `grad p`; ESI never solves the momentum matrices, it uses `A()` and `H()` | structure | translated: `A`, `H` of the assembled matrices on the current velocity |
| D4 | `H` omits the off-diagonal terms (the port's own doc says so) | structure | translated |
| D5 | drag split explicitly (`+Kd U_other`); ESI eliminates it partially (`rAUKd`, `phir`, `Ur`) | stiffness: dt/tau = 1.5 at step 1 | translated |
| D6 | dynamic pressure with the body force in the cell momentum equation; ESI uses `p_rgh` and the face flux `phig` from `snGrad(rho)` and `(rho1 - rho2) g` | structure | translated |
| D7 | phase-intensive equations; ESI has `ddt(alpha rho, U)`, `contErr` on the diagonal | structure | translated |
| D8 | the explicit part of the laminar stress, `div(alpha mu dev2(T(grad U)))`, is absent | first divergent operation at step 2 once D1-D7 were in: `HbyA1` off by 1.4e-5 relative; with the term, 4e-12 | translated, incl. the corrected wall-patch gradient |
| D9 | no virtual mass | gap in the dev form | translated for constant `Cvm`: `Vm = alpha_d Cvm rho_c`, the `Vm (ddt(U) + div(phi,U) - Sp(div phi) - DDtU_other)` terms and `DDtU` |

Test-set-up errors of mine, found by the same differential, NOT port defects:
(a) the first ESI-form run gave `rAU` off by 2.3e-6 relative; cause: the test
put a fixed-value velocity condition on the `empty` patch too, which adds wall
shear on the 2-D front/back faces (about a 6 % error in the Laplacian diagonal);
fixed in the test. (b) The hand estimates (below).

### Table 2: the ESI-form step vs ESI (pinned in the tests at 1e-12 alpha, 1e-10 of peak elsewhere)
Same metrics. "Cvm" rows use the case with `constantCoefficient` virtual mass.

| case | step | alpha (abs) | U.air | U.water | p |
|---|---|---|---|---|---|
| dt 1e-3 | 1 | 0 (72/72 bit-identical) | 4.3e-12 | 1.2e-13 | 5.2e-13 |
| dt 1e-3 | 2 | 8.7e-15 | 1.0e-12 | 1.6e-13 | 9.6e-14 |
| dt 1e-3 | 5 | 9.8e-15 | 1.7e-13 | 8.2e-14 | 3.5e-14 |
| dt 1e-3 | 10 | 9.6e-15 | 1.5e-13 | 4.6e-14 | 7.1e-14 |
| dt 1e-4 | 1 | 0 | 1.5e-11 | 1.8e-13 | 2.3e-12 |
| dt 1e-4 | 10 | 9.4e-16 | 8.4e-12 | 1.0e-13 | 5.6e-12 |
| Cvm 0.5, dt 1e-3 | 1 | 0 | 1.4e-13 | 3.8e-13 | 2.5e-14 |
| Cvm 0.5, dt 1e-3 | 10 | 1.4e-16 | 3.7e-14 | 1.0e-13 | 2.2e-13 |

All intermediates at steps 1 and 2 agree to 1e-11 of their peak (`Kd` 72/72
bit-identical at step 1; `rAU`, `phiHbyA`, `phig`, `rAUf` to 4e-16 at step 1;
`p_rgh` 1.5e-14, `phi1` 8.5e-12 at step 1). The remaining ~1e-12 is not
isolated: it grows slowly with step count and the linear solvers differ
(ESI diagonal-PCG at a normalised 1e-12, port DIC-CG at 1e-14, with the
reference cell pinned by doubling the diagonal, as upstream's `setReference`).
Bit-identical cells: alpha 72/72 at step 1 only; later steps 0 to 18 of 72;
`U`, `p`: none.

Ablations (each translated piece carries agreement): without the MULES
limiter alpha at step 2 is off by 3.2e-4; without the explicit stress term
`U.air` at step 2 is off by 4.8e-7 of peak (6.7e-8 absolute) where the full
step gives 1e-12; with `Cvm = 0.5` in the reference and `Cvm = 0` in the port
`U.air` at step 1 is off by 6.6e-2 m/s against a field peak of 1.6e-2 m/s.

### The ESI case needed three corrections to be a usable reference
1. **Energy equations.** With the tutorial's compressible-looking `eConst`
   and `rhoConst`, ESI's energy equation blew up (negative T at step 8, also
   with `Cv = 1e7`). They cannot influence a `rhoConst` hydrodynamics, so the
   dump copy of the solver skips `EEqns.H` (in the committed diff).
2. **The closed tank makes ESI's pressure matrix singular** (`rhoConst`:
   no `ddt(p_rgh)` term; `twoPhaseEulerFoam` has no `setReference` call).
   `DICPCG` (the tutorial's `GAMG`/`DIC` family) stalled or went wrong in
   some steps at tolerance 1e-15 (residuals 1e-6 to 1e1 after the iteration
   cap), and then the reference itself blew up at step 6 (dt = 1e-3), which I
   first took for physics. Diagnosed from the solver log; fixed in the case
   (not in the port) by `PCG` with a `diagonal` preconditioner at tolerance
   1e-12, and by moving the pressure level to 0 with `pMin -1e9` (at level 1e5
   the converged residual floor was 1e-3 Pa). The pressure solves then converge in
   about 31-32 iterations each. This means ESI's own closed-domain behaviour is fragile;
   not changed upstream, documented here.
3. The first dt = 1e-3 reference was therefore not stable beyond step 5; the
   committed one is, for 10 steps at both dt.

### Predictions scorecard (written before any run)
| | prediction | outcome |
|---|---|---|
| P1 | dev-form step-1 U.air, U.water within 25 % | **missed**: 46 % and 47 % |
| P2 | dev-form step-1 alpha between 1e-6 and 3e-4 | hit: 2.6e-4 (the cause is D1, not the explicit/implicit upwind difference I guessed: ESI's alpha at step 1 is the initial field) |
| P3 | dev-form step-1 p within 10 % of range | hit: 0.4 %; at step 10 it is 12 % |
| P4 | no bit-identical cells | hit |
| P5 | dev-form U.air error not smaller at step 10; alpha under 2e-3 | **missed both**: relative error fell from 46 % to 23 % (absolute grew 1.7x); alpha 5.5e-3 |
| P6 | the `dev2(T(grad U))` term is under 1e-3 of the balance and cannot be the first divergence | half: it is 1.4e-5 of `H` (under 1e-3), but it WAS the first divergent operation once D1-D7 were in |
| P7 | 10x smaller dt: alpha error down at least 3x, U.air error not | alpha hit (100x); U.air missed as worded (absolute 9x smaller because the velocity scales with dt) but the relative error is unchanged (46 % to 49 %), which is what was meant |
| P8 | ESI-form: 1e-10 of peak at step 1, 1e-9 at step 10 | hit, with margin: worst over both dt, all 10 steps and all fields 1.5e-11 |
| P9 | ESI-form: under 25 % bit-identical | missed by one cell count at one step: 18/72 = 25.0 % alpha at step 2 (dt = 1e-4); alpha 72/72 at step 1 (unchanged field) |
| P10 | first divergence would be the pressure reference or `ddtPhiCorr` | **missed**: the MULES limiter (step 2 alpha), the explicit stress term (step 2 `H`), ESI's own singular pressure solve, and my `empty` BC |

The hand estimate was also wrong: I derived a terminal slip of 0.30 m/s and a
drag time of 3.6e-5 s from the Newton regime. `Kd` at step 1 uses `U = 0`, so
it is the Stokes value (452 kg/m3/s, not 3.3e4 x alpha), the drag time is
6.7e-4 s (dt/tau = 1.5, not 28), and the step-1 slip is 0.08 m/s.

### Branches the case does NOT exercise (translated but unverified)
`ddtPhiCorr` filter (needs alpha above 0.99 or below 0.01 in a cell),
`residualAlpha` clipping, the `dgdt` feedback (about 1e-16 here, `rhoConst`),
the compressibility terms (zero), non-wall patches (refused by name), more
than one outer or pressure corrector, `Vm` with blended models. They are in
the code because the upstream has them; nothing here checks them.

### Missing forces and models (gaps, not defects)
Of the closures the shipped `bubbleColumn` uses, the ESI form now has: drag
(Schiller-Naumann, via the existing `DragModel`, `Kd` bit-identical at step 1),
and constant virtual mass. Not ported: lift (`constantLiftCoefficient`,
`Moraga`, `TomiyamaLift`, `LegendreMagnaudet`), wall lubrication, turbulent
dispersion (the `D` and `F` terms of `phiF1/2` in `pEqn.H`), blending
(`linear`, `hyperbolic`) and the `segregated` drag, the `isothermal` bubble
diameter, `RanzMarshall` heat transfer in the energy equation, compressible
phases and the energy equations, `faceMomentum`, partial `pU` options
(`implicitPhasePressure`), turbulence models, inlet and outlet boundaries (the
tutorial's `pressureInletOutletVelocity`, `prghPressure`, `inletOutlet`),
`nOuterCorrectors 3`. Lift with a constant coefficient is the next smallest
(`F = Cl rho_c alpha_d (U_r x curl U_c)` enters `phiF` and the pressure
equation's flux), but it needs the `curl` boundary treatment and a second
reference; not done.

### What was not done
The tutorial case itself (inlet/outlet, blending, compressible phases);
anything past 10 steps; the Foundation `multiphaseEuler` module (not built);
the appbuilder `ReactingTwoPhaseEulerFoam` wrapper still calls the dev form
(wiring the ESI form into it, and its API mirror regeneration, are open).
