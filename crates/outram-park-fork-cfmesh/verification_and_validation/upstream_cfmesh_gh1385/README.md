# Upstream cfMesh, built and run: reference output (gh:#1385)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; the numbers here have not yet
been reviewed by a human.

**Upstream:** cfMesh, `github.com/DAFoam/cfmesh` at
`e944f6ba695fcefce73ad306074f6b3c0ea432ae` (2026-02-17, GPL-3.0-or-later),
built against OpenFOAM-v2506 (ESI, GPL-3.0-or-later) from the GitHub mirror
`github.com/ZhangYanTJU/OpenFOAM-ESI` tag `OpenFOAM-v2506`
(`615aae61d7e95e110ca842ca105c799d10f59178`).

This is the first time upstream cfMesh has been run in this workspace. The
record holds upstream's own output, for the Rust port to be compared against
(the comparison itself is #1385's next step and #1476's reader; no Rust is
written here).

## Status and how to resume

STATUS: in progress. Done: OpenFOAM-v2506 partial build. Next: cfMesh build, then the runs.

Everything is built **outside the repository**, under
`/home/user/upstream-build/`, and is lost with the container. To rebuild:

1. `apt-get update && apt-get install -y flex bison zlib1g-dev libfl-dev time`
2. `git clone --depth 1 --branch OpenFOAM-v2506 https://github.com/ZhangYanTJU/OpenFOAM-ESI.git /home/user/upstream-build/OpenFOAM-v2506`
3. Copy `scripts/of-env.sh` and `scripts/build-of.sh` from this folder to
   `/home/user/upstream-build/scripts/` and run `build-of.sh` (partial build,
   below).
4. Clone cfMesh into the crate's gitignored `upstream_source/cfmesh_dafoam`
   (`upstream_source/README.md`) and run `scripts/build-cfmesh.sh`.
5. `scripts/run-case.sh <name> <tutorial dir> <app> [setup]` per case
   (commands in the results table).

## Predictions (written 2026-11-11, before any upstream run)

Read from upstream before the run:
`cartesianMeshGenerator::optimiseFinalMesh` and
`voronoiMeshGenerator::optimiseFinalMesh` call
`meshSurfaceOptimizer::optimizeSurface()` directly
(`meshSurfaceOptimizerOptimizeSurface.C:582`), which re-maps **only the edge
points** (`mapEdgeNodes`, line 636) and then smooths the remaining surface
points (`smoothLaplacianFC`, `smoothSurfaceOptimizer`, lines 650–652) with no
mapping after. `tetMeshGenerator::optimiseFinalMesh` instead calls
`meshOptimizer::optimizeSurface(octree)`
(`meshOptimizerOptimizeSurface.C:35`), which ends with
`meshSurfaceMapper::mapVerticesOntoSurfacePatches()`.

- **U1 (#1299, sphere, `cartesianMesh`).** Sphere R 3 m, 24 x 48 UV,
  `maxCellSize 0.5` (the #1279 setup S isotropic control). Upstream leaves
  boundary points **off** the surface: max distance of order 1e-3 m, within a
  factor 3 of the port's 2.891e-3 m.
- **U2 (#1299, sphere, `pMesh`).** Same: off the surface, within a factor 3
  of the port's 4.575e-3 m.
- **U3 (sphere, `tetMesh`).** On the surface to round-off: max distance below
  1e-12 m (port: 5.4e-16 m).
- **U4 (asmoOctree, `cartesianMesh`).** Points on the curved patches off the
  surface by up to a few percent of the local cell size; points on planar
  patches (inlet, outlet, symmetry plane) within 1e-9 of the bounding-box
  diagonal of their plane, since smoothing on a plane stays on it.
- **U5 (sphere, cell counts).** Upstream's counts equal the port's
  (`cartesian_mesh` 1128, `tet_mesh` 12840, `p_mesh` 1749). Least confident
  for `pMesh`, where `preMapVertices` is recorded as not ported (gh:#1273).

## Build

Machine: 4 cores (3 used: `WM_NCOMPPROCS=3`, another agent's cargo build on
the fourth), Ubuntu 24.04, g++ 13.3.0, no MPI.

### OpenFOAM-v2506, partial (2026-11-11)

`scripts/of-env.sh` sources `etc/bashrc` with `WM_MPLIB=dummy` (the dummy
Pstream, no MPI), `WM_COMPILE_OPTION=Opt` (`-O3`), double precision, 32-bit
labels. `scripts/build-of.sh` builds only what cfMesh and `checkMesh` link:

| target | why |
|---|---|
| `wmake/src` tools | wmake itself |
| `OSspecific/POSIX` (`libOSspecific.o`), `Pstream/dummy` | linked into `libOpenFOAM` |
| `OpenFOAM`, `fileFormats`, `surfMesh`, `meshTools` | cfMesh `meshLibrary/Make/options` |
| `finiteVolume` | cfMesh executables' `Make/options` |
| `mesh/blockMesh`, `mesh/extrudeModel`, `dynamicMesh` | `checkMesh` links `-ldynamicMesh`, which needs `extrudeModel`, which needs `blockMesh` |
| `applications/utilities/mesh/manipulation/checkMesh` | case 8 |

Not built: `foamToVTK` (it links `lagrangianIntermediate`, `conversion`,
`finiteArea` and `genericPatchFields`, most of the remaining tree; the polyMesh
files are what the comparison needs), ThirdParty, scotch, MPI, the rest of `src/`.

System packages installed: `flex bison zlib1g-dev libfl-dev` (and `time`, for
`/usr/bin/time -v`). No patch to OpenFOAM.

**One obstacle.** The first attempt passed `libso` to
`OSspecific/POSIX/Allwmake` and `Pstream/Allwmake`; that overrides their own
preferred target (`libo`, a `libOSspecific.o` object that `libOpenFOAM.so`
links), and the `libOpenFOAM.so` link failed (`cannot find
.../libOSspecific.o`). Calling the two `Allwmake` scripts with no argument,
as `src/Allwmake` does, fixed it; the rerun reused the compiled objects.

Wall time: 476 s (attempt 1, to the link error) + 2 843 s (attempt 2) =
**3 319 s (55 min)**, 1 607 compiler invocations. Disk: 864 MB in all
(297 MB source, 472 MB objects under `build/`, 90 MB libraries and binary
under `platforms/`). Log: `logs/build-openfoam-v2506.log.gz` (both attempts).

## Results

(filled in as the runs complete)
