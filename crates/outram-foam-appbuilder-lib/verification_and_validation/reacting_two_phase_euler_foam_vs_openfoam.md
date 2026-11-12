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
