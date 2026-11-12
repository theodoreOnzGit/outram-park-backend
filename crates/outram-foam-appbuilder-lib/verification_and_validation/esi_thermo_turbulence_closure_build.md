# ESI v2506 thermo / turbulence / fvOptions closure: measured build cost (gh:#1363 row 3, gh:#1361 step 4)

**Research, education and V&V only.** AI-run; not human-reviewed. Measured
2026-11-12 on 4 cores shared with another job, `WM_NCOMPPROCS=2`, gcc 13.3,
`-O3`, OpenFOAM-v2506 (`api=2506 patch=0`), dummy Pstream. Scripts, raw timings
and every edit: `upstream_openfoam_gh1363/scripts/build_closure/`
(`build_lib.sh`, `chain1.sh`, `chain2.sh`, `chain3.sh`, `timings_raw.txt`, the
`*.diff` files). Each library's object directory was deleted after it linked;
`df -h /` stayed above 3.5 GB (floor 3 GB).

## What was built, library by library
| library (ESI `src/`) | .C files | wall time | objects (transient) | `.so` kept |
|---|---:|---:|---:|---:|
| `thermophysicalModels/specie` | 33 | 48 s | 12 MB | 8.5 MB |
| `thermophysicalModels/thermophysicalProperties` | 54 | 46 s | 5 MB | 1.2 MB |
| `transportModels/compressible` | 1 | 2 s | 1 MB | 23 KB |
| `thermophysicalModels/basic` (libfluidThermophysicalModels) | 20 | 101 s | 27 MB | 15.7 MB |
| `TurbulenceModels/turbulenceModels` | 86 | 131 s | 18 MB | 3.6 MB |
| `TurbulenceModels/compressible` (pruned first, then rebuilt unmodified) | 7 | 47 s + 52 s | 6 MB | 3.3 MB |
| **subtotal: what the dump drivers need** | | **~7 min** | peak 27 MB | **32.3 MB** |
| `thermophysicalModels/solidSpecie` | 1 | 5 s | 1 MB | 0.34 MB |
| `thermophysicalModels/reactionThermo` (libreactionThermophysicalModels) | 18 | 176 s | 35 MB | 21.1 MB |
| `thermophysicalModels/solidThermo` | 5 | 37 s | 6 MB | 2.7 MB |
| `thermophysicalModels/SLGThermo` | 1 | 6 s | 1 MB | 0.05 MB |
| `thermophysicalModels/radiation` (libradiationModels) | 55 | 229 s | 15 MB | 2.7 MB |
| `transportModels/twoPhaseMixture` | 1 | 8 s | 1 MB | 0.22 MB |
| `transportModels/incompressible` | 13 | 72 s | 5 MB | 1.1 MB |
| `TurbulenceModels/incompressible` | 10 | 223 s | 18 MB | 3.9 MB |
| `dynamicFvMesh` | 11 | 62 s | 4 MB | 1.4 MB |
| `overset` | 28 | 140 s | 11 MB | 3.2 MB |
| `ODE` | 18 | 28 s | 1 MB | 0.29 MB |
| `regionModels/regionModel` (libregionModels) | 7 | 28 s | 2 MB | 0.62 MB |
| `fvOptions` | 51 | 308 s | 24 MB | 4.8 MB |
| **subtotal: GeN-Foam additions** | | **~22 min** | peak 35 MB | **42.4 MB** |
| **total, 19 libraries** | | **~29 min** | peak 35 MB | **74.7 MB** |
| `sonicFoam` (real binary, scratch options) | 1 | 21 s | small | 0.85 MB |
| `rhoPimpleFoam` (real binary, scratch options) | 1 | not timed | small | 1.0 MB |

Times are wall seconds measured by `build_lib.sh` while a merge-gate job shared
the CPU. Library sizes are `ls -l` of the `.so`.

## Edits (every one)
No `.C` or `.H` file was edited. `Make/options`/`Make/files` edits, all in
**scratch copies** (diffs in `scripts/build_closure/`):
1. `TurbulenceModels/compressible/Make/options` (`compressibleTurbulenceModels_options.diff`):
   `-I../turbulenceModels/lnInclude` made absolute, and `-lradiationModels
   -lsolidThermo -lsolidSpecie` removed, because those libraries were not built
   yet. **Superseded**: after the GeN-Foam additions the library was rebuilt
   in place from the unmodified tree (same 3,261,208-byte `.so`, 52 s); the
   committed drivers' scripts now point at the in-tree `lnInclude`.
2. Real `sonicFoam` (`sonicFoam_options.diff`, `sonicFoam_files.diff`): `-lthermoTools`
   removed from `EXE_LIBS`, `EXE` redirected to the scratch `bin/`. It links and
   runs, so `sonicFoam` does not use `thermoTools`.
3. Real `rhoPimpleFoam` (`rhoPimpleFoam_options.diff`, `rhoPimpleFoam_files.diff`):
   `-lthermoTools -ltopoChangerFvMesh -latmosphericModels -lregionFaModels -lfiniteArea`
   and `-I.../regionFaModels/lnInclude` removed, `EXE` redirected; the
   `overRhoPimpleDyMFoam` subfolder was not copied. It links and runs.

## What was not built (and what needs it)
`thermoTools` (needs `incompressibleMultiphaseSystems`, `combustionModels`,
`geometricVoF`, saturation models), `topoChangerFvMesh`, `atmosphericModels`,
`regionFaModels` + `finiteArea` (about 2.8 MB source, 196 .C), `genericPatchFields`
(needs `finiteArea`), `offbeat`/`sciantix`, the VoF/`interfaceProperties` libs.
GeN-Foam's `openFoamImportedSolvers` and `thermalHydraulics/solvers` need
`thermoTools`; that is the remaining cost for the real GeN-Foam binary.

## Correction of the #1361 scoping estimate
The scoping comment estimated the **thermo/turbulence/fvOptions closure** (tier A)
at 17-50 MB of libraries, 75-300 MB of transient objects and 10-30 min
(accurate "to about 3x", +-). Measured, for the same set pruned of `finiteArea`,
`thermoTools` and OFSolvers plus `regionModels`/`ODE`: **74.7 MB of libraries
(1.5x the high end), 35 MB peak transient objects (far below the low end: the
objects are deleted per library), ~29 min of wall time (the high end)**. The
library size was underestimated because `reactionThermophysicalModels` alone is
21 MB; time and transient size were over-estimated by 3-8x. The finiteVolume-like
template outlier feared for `compressibleTurbulenceModels` did not occur (47 s).

## Does it cover GeN-Foam 652b3da's thermal-hydraulics?
The link list in the scoping comment is covered, except what the table above
calls not built: specie, thermophysicalProperties, fluidThermo, solidThermo,
solidSpecie, reactionThermo, the compressible and incompressible transport libs,
the compressible and incompressible turbulence libs, radiationModels, SLGThermo,
fvOptions (+ overset, dynamicFvMesh), regionModels, ODE are built. `thermoTools`,
`genericPatchFields`/`finiteArea`, `offbeat` are not. **Not verified by building
GeN-Foam itself** (that is the next agent's step 5): a GeN-Foam source file that
needs one of the not-built libraries will fail to link, and the first GeN-Foam
build will show which.

## Real binaries versus the drivers (validation of the substitution)
With the fvOptions closure built, the real `sonicFoam` and `rhoPimpleFoam`
binaries (scratch link lines above) were run on the same 100-cell cases with the
same fixed dt and 700 steps as the drivers (`driver/sonicDump.C`,
`driver/rhoPimpleDump.C`, which omit `fvOptions`/MRF/mesh motion). The solver log
lines of the real binaries (`Solving for ..., Initial residual, Final residual,
No Iterations`, 17 digits, 6300 lines each) and the 1400 continuity-error lines
are **identical** to the drivers' output (`diff` empty). That is the evidence
that the drivers are the solvers on these cases; fields were not compared
directly between binary and driver (the written fields are not dumped as bit
patterns), only the residual histories, which would differ if any field did.
