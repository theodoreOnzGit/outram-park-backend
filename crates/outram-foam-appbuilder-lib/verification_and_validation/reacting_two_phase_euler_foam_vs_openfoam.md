# `reacting_two_phase_euler_foam` vs ESI v2506 reactingTwoPhaseEulerFoam / multiphaseEulerFoam (gh:#1363, row 5)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

## Status: NOT RUN. Blocked by disk and by a structural mismatch (2026-11-12)
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
5. **Isothermal in effect:** both phases 300 K, `RanzMarshall` kept (needed by
   the solver) but its driving temperature difference is zero. ESI still
   solves its energy equations; they cannot feed back with `rhoConst`.
6. **Constant diameters** (air 3 mm, water 0.1 mm) instead of the tutorial's
   `isothermal` air diameter (the port has no pressure-dependent diameter).
7. **Laminar, real viscosities** (air 1.84e-5, water 1e-3 Pa s). The port has
   no `dev2(T(grad U))` stress term; ESI has it (scheme `Gauss linear`). This
   is NOT removed from the case (nu = 0 would break the drag Reynolds number),
   so it is a known source of difference, bounded by prediction P6.
8. **All-upwind schemes, `Euler`, `nAlphaSubCycles 1`, `nAlphaCorr 1`,
   1 outer and 1 pressure corrector**, linear solvers at 1e-15, `relTol 0`.
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
