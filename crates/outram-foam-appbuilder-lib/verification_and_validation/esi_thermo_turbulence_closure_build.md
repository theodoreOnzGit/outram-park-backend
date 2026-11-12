# ESI v2506 thermo / turbulence closure for `rhoPimpleFoam` and `sonicFoam`: measured build cost (gh:#1363 row 3, gh:#1361 step 4)

**Research, education and V&V only.** AI-run; not human-reviewed. Measured
2026-11-12 on 4 cores shared with another job, `WM_NCOMPPROCS=2`, gcc 13.3,
`-O3`, OpenFOAM-v2506 (`api=2506 patch=0`), dummy Pstream. Scripts:
`upstream_openfoam_gh1363/scripts/build_closure/` (`build_lib.sh`, `chain1.sh`,
`chain2.sh`, raw `timings_raw.txt`).

## What was built (the minimum for the dump drivers)
Libraries were built in dependency order, and each library's object directory was
deleted after it linked (`build_lib.sh`).

| library (ESI `src/`) | .C files | wall time | objects (transient) | `.so` kept |
|---|---:|---:|---:|---:|
| `thermophysicalModels/specie` | 33 | 48 s | 12 MB | 8.5 MB |
| `thermophysicalModels/thermophysicalProperties` | 54 | 46 s | 5 MB | 1.2 MB |
| `transportModels/compressible` | 1 | 2 s | 1 MB | 23 KB |
| `thermophysicalModels/basic` (libfluidThermophysicalModels) | 20 | 101 s | 27 MB | 15.7 MB |
| `TurbulenceModels/turbulenceModels` | 86 | 131 s | 18 MB | 3.6 MB |
| `TurbulenceModels/compressible` (libcompressibleTurbulenceModels), scratch copy | 7 | 47 s | 6 MB | 3.3 MB |
| **total** | | **375 s (6.3 min)** | peak 27 MB | **32.3 MB** |

`df -h /` stayed above 4.8 GB throughout (floor 3 GB). The driver itself
(`sonicDump`) compiles in about 1 min.

## Edits (every one)
Only one source-tree file was touched in a **scratch copy**:
`TurbulenceModels/compressible/Make/options`
(`scripts/build_closure/compressibleTurbulenceModels_options.diff`):
- `-I../turbulenceModels/lnInclude` became `-I$(LIB_SRC)/TurbulenceModels/turbulenceModels/lnInclude` (the copy is not a sibling);
- `-lradiationModels`, `-lsolidThermo`, `-lsolidSpecie` removed from `LIB_LIBS`
  (not built; a grep of the library's sources finds no use of radiation or
  solidThermo, they are link-line / header-path leftovers for `makeTurbulenceModel`'s
  thermo types). The three libraries are not needed to *link* the drivers.
No `.C`/`.H` file was edited. The other five libraries were built in place with
unmodified `Make/options`.

## Correction of the #1361 estimate
The 2026-11-12 scoping estimate for tier A (thermo + turbulence + fvOptions
closure, pruned) was 17-50 MB of libraries, 75-300 MB transient objects and
10-30 min. The part of it that is **needed by the drivers** (everything except
`fvOptions` and its transitive closure) measured 32 MB, 27 MB peak transient
objects, 6.3 min: inside the low half of the range for libraries, and **below
the low end for time and transient size** (the template-heavy instantiations in
`compressibleTurbulenceModels` took 47 s, not the finiteVolume-like outlier the
estimate feared). The `fvOptions` closure (`fvOptions`, `reactionThermophysicalModels`,
`solidThermo`, `solidSpecie`, `radiationModels`, `SLGThermo`, `overset`,
`dynamicFvMesh`, `regionModels`, ...) was **not built**; it is what the real
`rhoPimpleFoam`/`sonicFoam` binaries additionally need
(`createFvOptions.H`, `fvOptions.H`, `-lfvOptions`, plus for `rhoPimpleFoam`
`dynamicFvMesh`, `topoChangerFvMesh`, `thermoTools`, `atmosphericModels`,
`regionFaModels` and `finiteArea`), so the drivers are used instead and
say so.

## What this closure serves
`psiThermo::New` / `fluidThermo::New` (all `hePsiThermo`/`heRhoThermo` pure-mixture
types registered in `libfluidThermophysicalModels`), `compressible::turbulenceModel::New`
(laminar and the RAS/LES models), `sutherland`, `const`, `hConst`, `perfectGas`,
`sensibleEnthalpy/InternalEnergy`. GeN-Foam step 4 can link the same six libraries.
